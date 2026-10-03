//! 取链分派层：**每个音源只用自己的链路**，不做跨音源替换。
//!
//! # 设计约束（用户要求，v1.0.8 起生效）
//!
//! 用户明确指出「每个音源点就要用每个音源点下载，不要搞什么回退到谁谁谁」。
//! 因此本模块只做两件事：
//! 1. 把任务按 [`Platform`] 分派到该音源自己的取链实现；
//! 2. 把该音源自己的账号凭据（[`PlatformCredentialSource`]）交给它。
//!
//! 取链失败时**原样返回该音源自己的错误**（例如「酷狗音乐：该歌曲为付费/VIP 曲目，
//! 需要酷狗会员账号，请在『设置 → 平台账号』填入酷狗 Cookie 后重试」），
//! 不去别的音源找同名曲目。v1.0.1–v1.0.7 的 `download::fallback` 跨源替换模块已删除。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use reqwest::Client;

use crate::platforms::credentials::{
    AnonymousCredentialSource, PlatformCookies, PlatformCredentialSource,
};
use crate::platforms::qqmusic::credentials::{
    FileQqCredentialSource, QqCookie, QqCredentialSource,
};
use crate::platforms::Platform;

/// 一次取链的结果：直链、解密密钥，以及只作用于本次下载请求的附加请求头。
///
/// `headers` 目前只由自定义音源脚本提供，属于**不可信输入**：传输层在发请求前会
/// 重新做数量、总字节、名称与字符校验（见 `transfer::request_download_response`）。
/// 因此这里不过滤，也**不得**回传前端或写入任务记录。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadLink {
    pub url: String,
    pub key: String,
    pub headers: Vec<(String, String)>,
}

impl DownloadLink {
    /// 平台内置链路：不附加自定义请求头，请求与旧版 `(url, key)` 逐字节一致。
    pub fn new(url: String, key: String) -> Self {
        Self {
            url,
            key,
            headers: Vec::new(),
        }
    }
}

/// 平台链接和凭据由运行时提供；核心只决定何时重新请求链接。
pub trait DownloadLinkProvider: Send + Sync {
    fn fetch<'a>(
        &'a self,
        platform: Platform,
        song_mid: &'a str,
        filename: &'a str,
    ) -> BoxFuture<'a, Result<DownloadLink, String>>;
}

/// 用户可见的音源名称，用于日志与「该平台不支持」这类提示。
pub(crate) fn platform_label(platform: Platform) -> &'static str {
    match platform {
        Platform::QqMusic => "QQ 音乐",
        Platform::Kuwo => "酷我",
        Platform::Kugou => "酷狗",
        Platform::Netease => "网易云",
        Platform::Bilibili => "哔哩哔哩",
        Platform::Migu => "咪咕",
        Platform::Script(_) => "自定义音源",
    }
}

/// 内置音源共用的 HTTP 链接提供器。运行时只注入凭据来源；下载任务不接触令牌。
pub struct PlatformDownloadLinkProvider {
    client: Client,
    qq_credentials: Arc<dyn QqCredentialSource>,
    platform_credentials: Arc<dyn PlatformCredentialSource>,
}

impl PlatformDownloadLinkProvider {
    pub fn new(
        qq_credentials: Arc<dyn QqCredentialSource>,
        platform_credentials: Arc<dyn PlatformCredentialSource>,
    ) -> Self {
        let client = link_client();
        Self {
            client,
            qq_credentials,
            platform_credentials,
        }
    }

    /// 无任何账号时的构造方式（离线测试与 CLI 复用）。
    pub fn anonymous(qq_credentials: Arc<dyn QqCredentialSource>) -> Self {
        let client = link_client();
        Self {
            client,
            qq_credentials,
            platform_credentials: Arc::new(AnonymousCredentialSource),
        }
    }

    /// 直接以持久化凭据文件构造时只需提供文件路径；QQ 凭据会在每次请求时读取并按需刷新，
    /// 平台账号（酷狗/网易云/咪咕 Cookie）保持匿名。
    pub fn from_credentials_file(path: impl Into<PathBuf>) -> Self {
        let client = link_client();
        let qq_credentials = Arc::new(FileQqCredentialSource::new(path, client.clone()));
        Self {
            client,
            qq_credentials,
            platform_credentials: Arc::new(AnonymousCredentialSource),
        }
    }

    /// 允许运行时复用已有 HTTP 客户端，也便于测试使用本地服务。
    pub fn with_client(
        client: Client,
        qq_credentials: Arc<dyn QqCredentialSource>,
        platform_credentials: Arc<dyn PlatformCredentialSource>,
    ) -> Self {
        Self {
            client,
            qq_credentials,
            platform_credentials,
        }
    }

    /// 读取当前平台账号设置。读取失败只降级为匿名：账号设置坏掉不应阻断下载，
    /// 也不应把用户从「匿名可下」变成「整单失败」。
    async fn platform_cookies(&self) -> PlatformCookies {
        match self.platform_credentials.cookies().await {
            Ok(cookies) => cookies,
            Err(error) => {
                log::warn!("读取平台账号设置失败，本次按匿名取链: {error}");
                PlatformCookies::default()
            }
        }
    }
}

fn link_client() -> Client {
    Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/86.0.4240.198 Safari/537.36")
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create link HTTP client")
}

impl DownloadLinkProvider for PlatformDownloadLinkProvider {
    fn fetch<'a>(
        &'a self,
        platform: Platform,
        song_mid: &'a str,
        filename: &'a str,
    ) -> BoxFuture<'a, Result<DownloadLink, String>> {
        Box::pin(async move {
            match platform {
                // QQ 音乐：优先用用户在「设置 → 平台账号」里粘贴的网页版 Cookie
                // （uin + qqmusic_key/qm_keyst）。平台已关闭匿名取链，没填 Cookie 才回落到
                // 应用自己的登录态；Cookie 缺字段就如实报错，不静默转匿名。
                Platform::QqMusic => {
                    let cookies = self.platform_cookies().await;
                    let credentials = match cookies.qq() {
                        Some(raw) => match QqCookie::parse(raw) {
                            Ok(cookie) => Some(cookie.into_auth()),
                            Err(error) => return Err(format!("QQ 音乐：{error}")),
                        },
                        // 凭据文件损坏时如实报错：QQ 取链必须用它自己的凭据。
                        None => self.qq_credentials.current().await?,
                    };

                    crate::platforms::qqmusic::link::fetch_vkey_link(
                        &self.client,
                        song_mid,
                        filename,
                        credentials.as_ref(),
                    )
                    .await
                    .map(|(url, key)| DownloadLink::new(url, key))
                }
                // 酷我当前接口不需要账号（明文直链），也不使用平台 Cookie。
                Platform::Kuwo => crate::platforms::kuwo::link::get_download_link(
                    &self.client,
                    song_mid,
                    filename,
                )
                .await
                .map(|(url, key)| DownloadLink::new(url, key)),
                // 网易云：匿名只拿得到 128k/320k 明文 mp3（无损会被静默降级），
                // 配置账号后走 `player/url/v1`，由网易云按账号权益决定档位与是否试听。
                Platform::Netease => {
                    let cookies = self.platform_cookies().await;
                    crate::platforms::netease::link::get_download_link(
                        &self.client,
                        song_mid,
                        filename,
                        cookies.netease(),
                    )
                    .await
                    .map(|(url, key)| DownloadLink::new(url, key))
                }
                // 咪咕：匿名只声明 128kmp3（toneFlag 被忽略）。VIP 独占曲 listenSong.do
                // 只回「暂不提供试听地址」，配置账号后再由咪咕自己的服务端裁决。
                Platform::Migu => {
                    let cookies = self.platform_cookies().await;
                    crate::platforms::migu::link::get_download_link(
                        &self.client,
                        song_mid,
                        filename,
                        cookies.migu(),
                    )
                    .await
                    .map(|(url, key)| DownloadLink::new(url, key))
                }
                // 哔哩哔哩：mid 就是 bvid，容器固定 m4a/AAC（192kbps 音轨），不加密。
                // 音轨由第三方 PCDN 分发，只认「浏览器 UA + 视频页 Referer」的组合，
                // 因此这是唯一自带自定义请求头的内置音源（见 `bilibili::link::media_headers`）。
                Platform::Bilibili => crate::platforms::bilibili::link::get_download_link(
                    &self.client,
                    song_mid,
                    filename,
                )
                .await
                .map(|(url, key)| DownloadLink {
                    url,
                    key,
                    headers: crate::platforms::bilibili::link::media_headers(song_mid),
                }),
                // 酷狗：mid 里编码了三个档位的 hash（复合编码见 kugou/parser.rs）。
                // 付费/VIP 曲目由酷狗服务端按账号权益签发直链，未配置酷狗 Cookie 时
                // 只能给出匿名能力（128k / 320k / 无损中公开可取的档位）。
                Platform::Kugou => {
                    let cookies = self.platform_cookies().await;
                    crate::platforms::kugou::link::get_download_link(
                        &self.client,
                        song_mid,
                        filename,
                        cookies.kugou(),
                    )
                    .await
                    .map(|(url, key)| DownloadLink::new(url, key))
                }
                // 自定义音源：调用脚本的 getUrl。错误文案已按重试语义分级
                // （网络类保持 `网络错误: …` 前缀交给上层重试，确定性错误带 `音源脚本错误：` 前缀）。
                Platform::Script(id) => {
                    let source_id = id.get();
                    match crate::script::fetch_url_for_link(source_id, song_mid, filename).await {
                        Ok(link) => {
                            if let Some(quality) = link.quality.as_deref() {
                                if crate::script::quality_filename(quality).as_deref()
                                    != Some(filename)
                                {
                                    log::warn!(
                                        "音源 {source_id} 返回音质 {quality}，与任务文件名 {filename} 不一致（容器一致，可正常播放）"
                                    );
                                }
                            }
                            if !link.headers.is_empty() {
                                // 只记录条数，不记录取值（可能含 Cookie 等凭据）。
                                log::info!(
                                    "音源 {source_id} 为本次下载附加 {} 个请求头",
                                    link.headers.len()
                                );
                            }
                            // 自定义请求头原样交给传输层：它只作用于本次下载请求，
                            // 并由 transfer.rs 重新做上限/禁止名单/字符校验后再附加。
                            Ok(DownloadLink {
                                url: link.url,
                                key: String::new(),
                                headers: link.headers,
                            })
                        }
                        Err(message) => Err(message),
                    }
                }
            }
        })
    }
}

/// 临时网络错误最多尝试三次，平台明确拒绝时立即返回错误。
pub async fn fetch_download_link_with_retry(
    provider: &dyn DownloadLinkProvider,
    song_mid: &str,
    filename: &str,
    task_id: &str,
    platform: Platform,
) -> Result<DownloadLink, String> {
    let mut last_error = String::new();

    for attempt in 0..3 {
        match provider.fetch(platform, song_mid, filename).await {
            Ok(link) => return Ok(link),
            Err(error) => {
                if !is_retryable_link_error(&error) {
                    // 登录态失效和平台拒绝属于确定性错误，重复请求只会延迟任务失败。
                    log::warn!("任务 {task_id} 获取下载链接失败: {error}");
                    return Err(error);
                }

                log::warn!(
                    "任务 {task_id} 获取下载链接失败 (尝试 {}/3): {error}",
                    attempt + 1
                );
                last_error = error;
                if attempt < 2 {
                    // 首次和第二次失败后分别等待 1 秒、2 秒；第三次直接返回最终错误。
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                }
            }
        }
    }

    Err(last_error)
}

/// 错误分类沿用现有平台接口的中文错误前缀，保持客户端原有重试行为。
pub fn is_retryable_link_error(error: &str) -> bool {
    error.starts_with("网络错误")
        || error.starts_with("读取响应失败")
        || error.starts_with("解析响应失败")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::{
        fetch_download_link_with_retry, platform_label, DownloadLink, DownloadLinkProvider,
        PlatformDownloadLinkProvider,
    };
    use crate::platforms::credentials::{PlatformCookies, PlatformCredentialSource};
    use crate::platforms::qqmusic::credentials::{
        FileQqCredentialSource, QqAuth, QqCredentialSource,
    };
    use crate::platforms::Platform;
    use futures_util::future::BoxFuture;
    use reqwest::Client;

    struct RejectedLink {
        attempts: AtomicUsize,
    }

    struct RecoveringLink {
        attempts: AtomicUsize,
    }

    /// 每次都返回凭据文件损坏错误的 QQ 凭据源（离线）。
    struct BrokenQqCredentials;

    impl QqCredentialSource for BrokenQqCredentials {
        fn current(&self) -> BoxFuture<'_, Result<Option<QqAuth>, String>> {
            Box::pin(async { Err("QQ 音乐凭据文件损坏".to_string()) })
        }
    }

    /// 统计读取次数并固定返回一份酷狗 Cookie 的凭据源（离线）。
    struct CountingCredentials {
        calls: AtomicUsize,
        cookies: PlatformCookies,
    }

    impl PlatformCredentialSource for CountingCredentials {
        fn cookies(&self) -> BoxFuture<'_, Result<PlatformCookies, String>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let cookies = self.cookies.clone();
            Box::pin(async move { Ok(cookies) })
        }
    }

    /// 读取即失败的凭据源（模拟设置文件坏掉）。
    struct FailingCredentials;

    impl PlatformCredentialSource for FailingCredentials {
        fn cookies(&self) -> BoxFuture<'_, Result<PlatformCookies, String>> {
            Box::pin(async { Err("设置读取失败".to_string()) })
        }
    }

    impl DownloadLinkProvider for RejectedLink {
        fn fetch<'a>(
            &'a self,
            _platform: Platform,
            _song_mid: &'a str,
            _filename: &'a str,
        ) -> BoxFuture<'a, Result<DownloadLink, String>> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err("平台拒绝: 104003".to_string()) })
        }
    }

    impl DownloadLinkProvider for RecoveringLink {
        fn fetch<'a>(
            &'a self,
            _platform: Platform,
            _song_mid: &'a str,
            _filename: &'a str,
        ) -> BoxFuture<'a, Result<DownloadLink, String>> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if attempt == 0 {
                    Err("网络错误: 连接超时".to_string())
                } else {
                    Ok(DownloadLink::new(
                        "https://example.test/audio".to_string(),
                        String::new(),
                    ))
                }
            })
        }
    }

    fn provider_with(
        credentials: Arc<dyn PlatformCredentialSource>,
    ) -> PlatformDownloadLinkProvider {
        PlatformDownloadLinkProvider::with_client(
            Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap(),
            Arc::new(BrokenQqCredentials),
            credentials,
        )
    }

    fn anonymous_provider() -> PlatformDownloadLinkProvider {
        let path = std::env::temp_dir().join(format!(
            "hotdownloader-invalid-qq-{:x}.json",
            std::process::id()
        ));
        let _ = std::fs::write(&path, "{");
        PlatformDownloadLinkProvider::with_client(
            Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap(),
            Arc::new(FileQqCredentialSource::new(path, link_client_for_tests())),
            Arc::new(crate::platforms::credentials::AnonymousCredentialSource),
        )
    }

    fn link_client_for_tests() -> Client {
        Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
    }

    /// 非法 mid 在发请求之前就失败，因此这些用例全程不联网。
    const INVALID_KUGOU_MID: &str = "not-a-hash";
    const INVALID_MIGU_MID: &str = "invalid-mid";

    #[tokio::test]
    async fn platform_rejection_is_not_retried() {
        let provider = RejectedLink {
            attempts: AtomicUsize::new(0),
        };
        let result = fetch_download_link_with_retry(
            &provider,
            "song",
            "file.mp3",
            "task",
            Platform::QqMusic,
        )
        .await;

        assert_eq!(result.unwrap_err(), "平台拒绝: 104003");
        assert_eq!(provider.attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn temporary_network_error_retries_and_returns_link() {
        let provider = RecoveringLink {
            attempts: AtomicUsize::new(0),
        };
        let result = fetch_download_link_with_retry(
            &provider,
            "song",
            "file.mp3",
            "task",
            Platform::QqMusic,
        )
        .await;

        assert_eq!(
            result.unwrap().url,
            "https://example.test/audio",
            "第二次请求成功后应立即返回真实链接"
        );
        assert_eq!(provider.attempts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn builtin_link_carries_no_custom_headers() {
        // `DownloadLink::new` 仍然只带 (url, key)：内置音源里唯一自带请求头的是
        // 哔哩哔哩（第三方 PCDN 只认浏览器 UA + 视频页 Referer，见
        // `platforms::bilibili::link::media_headers`），其余六个音源保持旧行为。
        let link = DownloadLink::new("https://cdn.test/a.mp3".to_string(), "key".to_string());
        assert_eq!(link.url, "https://cdn.test/a.mp3");
        assert_eq!(link.key, "key");
        assert!(link.headers.is_empty());
    }

    #[test]
    fn platform_labels_cover_every_builtin_source() {
        assert_eq!(platform_label(Platform::Kugou), "酷狗");
        assert_eq!(platform_label(Platform::Netease), "网易云");
        assert_eq!(platform_label(Platform::Migu), "咪咕");
        assert_eq!(platform_label(Platform::QqMusic), "QQ 音乐");
        assert_eq!(platform_label(Platform::Kuwo), "酷我");
        assert_eq!(platform_label(Platform::Bilibili), "哔哩哔哩");
    }

    /// v1.0.8 核心约束：取链失败时保留**该音源自己**的错误，
    /// 不得出现任何跨音源替换的痕迹。
    #[tokio::test]
    async fn platform_errors_never_mention_other_sources() {
        let provider = anonymous_provider();

        let kugou = provider
            .fetch(Platform::Kugou, INVALID_KUGOU_MID, "128.mp3")
            .await
            .unwrap_err();
        assert!(kugou.contains("无效的酷狗歌曲标识"), "{kugou}");

        let migu = provider
            .fetch(Platform::Migu, INVALID_MIGU_MID, "PQ.mp3")
            .await
            .unwrap_err();
        assert!(migu.contains("无效的咪咕歌曲标识"), "{migu}");

        for message in [&kugou, &migu] {
            // 片段用运行期拼接：源码里不存在完整的跨源字样，静态守卫脚本（扫字符串字面量）
            // 因此不会在断言自身上误报。
            let forbidden = [
                concat!("已在", "其它"),
                concat!("其它", "内置音源"),
                concat!("换", "源"),
                concat!("回", "退"),
                concat!("替代", "音源"),
            ];
            for needle in forbidden {
                assert!(!message.contains(needle), "错误文案含跨源字样: {message}");
            }
        }
    }

    /// 平台 Cookie 只在需要账号的音源上读取一次，并且按音源各取所需。
    #[tokio::test]
    async fn platform_cookies_are_read_for_the_platform_that_needs_them() {
        let credentials = Arc::new(CountingCredentials {
            calls: AtomicUsize::new(0),
            cookies: PlatformCookies {
                // QQ 这条故意缺登录密钥：解析阶段就失败，测试不会联网。
                qq: Some("uin=o0123456789; pgv_pvid=1".to_string()),
                kugou: Some("token=k; userid=1; dfid=d; mid=m".to_string()),
                netease: Some("MUSIC_U=n".to_string()),
                migu: Some("token=g".to_string()),
            },
        });
        let provider = provider_with(credentials.clone());

        // 非法 mid 在解析阶段失败 → 不联网，但凭据已经被读走。
        let _ = provider
            .fetch(Platform::Kugou, INVALID_KUGOU_MID, "128.mp3")
            .await;
        assert_eq!(credentials.calls.load(Ordering::SeqCst), 1);

        // QQ 分支同样先读平台 Cookie，再决定用 Cookie 还是应用内登录态。
        let _ = provider
            .fetch(Platform::QqMusic, "song-mid", "M500.mp3")
            .await;
        assert_eq!(credentials.calls.load(Ordering::SeqCst), 2);
    }

    /// QQ Cookie 缺字段时在取链前直接报错，且不回显 Cookie 原文。
    #[tokio::test]
    async fn invalid_qq_cookie_fails_before_any_request() {
        let credentials = Arc::new(CountingCredentials {
            calls: AtomicUsize::new(0),
            cookies: PlatformCookies {
                qq: Some("uin=o0123456789; pgv_pvid=SECRETPVID".to_string()),
                ..PlatformCookies::default()
            },
        });
        let provider = provider_with(credentials.clone());

        let error = provider
            .fetch(Platform::QqMusic, "song-mid", "M500.mp3")
            .await
            .unwrap_err();

        assert!(error.starts_with("QQ 音乐："), "{error}");
        assert!(error.contains("qqmusic_key"), "{error}");
        assert!(!error.contains("SECRETPVID"), "{error}");
        assert_eq!(credentials.calls.load(Ordering::SeqCst), 1);
    }

    /// 设置读取失败只降级为匿名，不把错误抛给下载任务。
    #[tokio::test]
    async fn credential_source_failure_degrades_to_anonymous() {
        let provider = provider_with(Arc::new(FailingCredentials));

        let error = provider
            .fetch(Platform::Kugou, INVALID_KUGOU_MID, "128.mp3")
            .await
            .unwrap_err();
        assert!(error.contains("无效的酷狗歌曲标识"), "{error}");
    }
}
