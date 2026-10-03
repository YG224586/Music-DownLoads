use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use reqwest::Client;

use crate::platforms::qqmusic::credentials::{FileQqCredentialSource, QqCredentialSource};
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

/// 两个平台共用的 HTTP 链接提供器。运行时只注入凭据来源；下载任务不接触令牌。
pub struct PlatformDownloadLinkProvider {
    client: Client,
    qq_credentials: Arc<dyn QqCredentialSource>,
}

impl PlatformDownloadLinkProvider {
    pub fn new(qq_credentials: Arc<dyn QqCredentialSource>) -> Self {
        let client = link_client();
        Self {
            client,
            qq_credentials,
        }
    }

    /// 直接以持久化凭据文件构造时只需提供文件路径；凭据会在每次请求时读取并按需刷新。
    pub fn from_credentials_file(path: impl Into<PathBuf>) -> Self {
        let client = link_client();
        let qq_credentials = Arc::new(FileQqCredentialSource::new(path, client.clone()));
        Self {
            client,
            qq_credentials,
        }
    }

    /// 允许运行时复用已有 HTTP 客户端，也便于测试使用本地服务。
    pub fn with_client(client: Client, qq_credentials: Arc<dyn QqCredentialSource>) -> Self {
        Self {
            client,
            qq_credentials,
        }
    }

    /// 主平台确定性失败后的内置多音源回退。
    ///
    /// 只有「重试也救不回来」的错误才走回退：网络类错误仍交给
    /// [`fetch_download_link_with_retry`] 重试，保持原有重试语义。
    /// 回退失败时不改变用户可见的主平台错误（找不到匹配曲目时给更明确的文案）。
    async fn fallback_after_failure(
        &self,
        platform: Platform,
        song_mid: &str,
        filename: &str,
        primary_error: String,
    ) -> Result<DownloadLink, String> {
        use crate::download::fallback::{self, FallbackOutcome};

        if is_retryable_link_error(&primary_error) {
            return Err(primary_error);
        }

        log::warn!("歌曲 {song_mid} 主平台取链失败（{primary_error}），尝试内置多音源回退");

        match fallback::fetch_from_other_sources(&self.client, platform, song_mid, filename).await {
            FallbackOutcome::Linked(link) => {
                log::info!(
                    "歌曲 {song_mid} 从{}回退到{}成功，实际音质 {}",
                    fallback::platform_label(platform),
                    fallback::platform_label(link.source),
                    link.quality
                );
                Ok(DownloadLink::new(link.url, link.key))
            }
            FallbackOutcome::Unavailable(reason) => {
                log::warn!("歌曲 {song_mid} 内置回退失败: {reason}");
                // 回退失败时不改变用户可见的主平台错误；只有 QQ 的原始错误
                // （如 104003「无法获取下载链接」）对用户没有指导意义，换成明确文案。
                Err(fallback::no_match_message(platform, &primary_error))
            }
            FallbackOutcome::Transient(reason) => {
                log::warn!("歌曲 {song_mid} 内置回退遇到临时错误: {reason}");
                Err(primary_error)
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
                Platform::QqMusic => {
                    // 未登录（匿名）时 QQ 只会拒绝付费曲目和 320k/flac 等高音质，
                    // 这类确定性失败由内置多音源回退接手，不再直接失败。
                    let primary = match self.qq_credentials.current().await {
                        Ok(credentials) => {
                            crate::platforms::qqmusic::link::fetch_vkey_link(
                                &self.client,
                                song_mid,
                                filename,
                                credentials.as_ref(),
                            )
                            .await
                        }
                        // 凭据文件损坏同样是确定性失败，回退比直接失败更有用。
                        Err(error) => Err(error),
                    };

                    match primary {
                        Ok((url, key)) => Ok(DownloadLink::new(url, key)),
                        Err(error) => {
                            self.fallback_after_failure(
                                Platform::QqMusic,
                                song_mid,
                                filename,
                                error,
                            )
                            .await
                        }
                    }
                }
                // 酷我当前接口不使用 QQ 登录态，不能因 QQ 凭据文件出错而阻止酷我任务。
                Platform::Kuwo => crate::platforms::kuwo::link::get_download_link(
                    &self.client,
                    song_mid,
                    filename,
                )
                .await
                .map(|(url, key)| DownloadLink::new(url, key)),
                // 网易云：匿名只拿得到 128k/320k 明文 mp3（无损会被静默降级，
                // 因此 qualities 里不提供 flac/hires），付费曲返回试听片段会被拒绝。
                // 付费/试听类确定性失败由内置多音源回退接手。
                Platform::Netease => {
                    let primary = crate::platforms::netease::link::get_download_link(
                        &self.client,
                        song_mid,
                        filename,
                    )
                    .await;

                    match primary {
                        Ok((url, key)) => Ok(DownloadLink::new(url, key)),
                        Err(error) => {
                            self.fallback_after_failure(
                                Platform::Netease,
                                song_mid,
                                filename,
                                error,
                            )
                            .await
                        }
                    }
                }
                // 咪咕：匿名只声明 128kmp3（toneFlag 被忽略，实测所有档位同一条明文 mp3）。
                // VIP 独占曲目 listenSong.do 只回「暂不提供试听地址」，这类确定性失败
                // 由内置多音源回退接手（`resourceinfo.do` 仍能反查出标题/歌手/时长）。
                Platform::Migu => {
                    let primary = crate::platforms::migu::link::get_download_link(
                        &self.client,
                        song_mid,
                        filename,
                    )
                    .await;

                    match primary {
                        Ok((url, key)) => Ok(DownloadLink::new(url, key)),
                        Err(error) => {
                            self.fallback_after_failure(Platform::Migu, song_mid, filename, error)
                                .await
                        }
                    }
                }
                // 哔哩哔哩：mid 就是 bvid，容器固定 m4a/AAC（192kbps 音轨），不加密。
                Platform::Bilibili => crate::platforms::bilibili::link::get_download_link(
                    &self.client,
                    song_mid,
                    filename,
                )
                .await
                .map(|(url, key)| DownloadLink::new(url, key)),
                // 酷狗：mid 里编码了三个档位的 hash（复合编码见 kugou/parser.rs）。
                // 付费/VIP 曲目匿名拿不到直链（实测服务端签发权益），
                // 这类确定性失败由内置多音源回退接手。
                Platform::Kugou => {
                    let primary = crate::platforms::kugou::link::get_download_link(
                        &self.client,
                        song_mid,
                        filename,
                    )
                    .await;

                    match primary {
                        Ok((url, key)) => Ok(DownloadLink::new(url, key)),
                        Err(error) => {
                            self.fallback_after_failure(Platform::Kugou, song_mid, filename, error)
                                .await
                        }
                    }
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

    use super::{
        fetch_download_link_with_retry, DownloadLink, DownloadLinkProvider,
        PlatformDownloadLinkProvider,
    };
    use crate::platforms::Platform;
    use futures_util::future::BoxFuture;

    struct RejectedLink {
        attempts: AtomicUsize,
    }

    struct RecoveringLink {
        attempts: AtomicUsize,
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
        // 内置链路（QQ/酷我及多音源回退）必须与旧版 (url, key) 行为一致：
        // 不携带任何自定义请求头，传输层因此得到与改动前逐字节相同的请求。
        let link = DownloadLink::new("https://cdn.test/a.mp3".to_string(), "key".to_string());
        assert_eq!(link.url, "https://cdn.test/a.mp3");
        assert_eq!(link.key, "key");
        assert!(link.headers.is_empty());
    }

    #[tokio::test]
    async fn kugou_and_migu_rejections_route_through_the_builtin_fallback() {
        let path = std::env::temp_dir().join(format!(
            "hotdownloader-invalid-qq-{:x}.json",
            rand::random::<u64>()
        ));
        tokio::fs::write(&path, "{").await.unwrap();
        let provider = PlatformDownloadLinkProvider::from_credentials_file(&path);

        // 非法 mid 在主平台与回退链路里都在发请求之前失败，因此这个测试全程不联网：
        // 它证明酷狗/咪咕的确定性失败确实走到了内置回退（错误文案里带上了回退结论），
        // 并且没有去读 QQ 凭据文件。
        let kugou = provider
            .fetch(Platform::Kugou, "not-a-hash", "128.mp3")
            .await
            .unwrap_err();
        assert!(kugou.contains("无效的酷狗歌曲标识"), "{kugou}");
        assert!(kugou.contains("已在其它内置音源搜索"), "{kugou}");

        let migu = provider
            .fetch(Platform::Migu, "invalid-mid", "PQ.mp3")
            .await
            .unwrap_err();
        assert!(migu.contains("无效的咪咕歌曲标识"), "{migu}");
        assert!(migu.contains("已在其它内置音源搜索"), "{migu}");

        tokio::fs::remove_file(path).await.unwrap();
    }
}
