use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use reqwest::Client;

use crate::platforms::qqmusic::credentials::{FileQqCredentialSource, QqCredentialSource};
use crate::platforms::Platform;

/// 平台链接和凭据由运行时提供；核心只决定何时重新请求链接。
pub trait DownloadLinkProvider: Send + Sync {
    fn fetch<'a>(
        &'a self,
        platform: Platform,
        song_mid: &'a str,
        filename: &'a str,
    ) -> BoxFuture<'a, Result<(String, String), String>>;
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
    ) -> Result<(String, String), String> {
        use crate::download::fallback::{self, FallbackOutcome};

        if is_retryable_link_error(&primary_error) {
            return Err(primary_error);
        }

        log::warn!("歌曲 {song_mid} 主平台取链失败（{primary_error}），尝试内置多音源回退");

        match fallback::fetch_from_other_sources(&self.client, platform, song_mid, filename).await {
            FallbackOutcome::Linked(link) => {
                log::info!(
                    "歌曲 {song_mid} 从 QQ 音乐回退到{}成功，实际音质 {}",
                    fallback::platform_label(link.source),
                    link.quality
                );
                Ok((link.url, link.key))
            }
            FallbackOutcome::Unavailable(reason) => {
                log::warn!("歌曲 {song_mid} 内置回退失败: {reason}");
                if matches!(platform, Platform::QqMusic) {
                    // QQ 的原始错误（如 104003「无法获取下载链接」）对用户没有指导意义，
                    // 换成「需要登录或曲目受限 + 没找到替代」的说明。
                    Err(fallback::NO_MATCH_MESSAGE.to_string())
                } else {
                    Err(primary_error)
                }
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
    ) -> BoxFuture<'a, Result<(String, String), String>> {
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
                        Ok(link) => Ok(link),
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
                Platform::Kuwo => {
                    crate::platforms::kuwo::link::get_download_link(
                        &self.client,
                        song_mid,
                        filename,
                    )
                    .await
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
) -> Result<(String, String), String> {
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
        fetch_download_link_with_retry, DownloadLinkProvider, PlatformDownloadLinkProvider,
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
        ) -> BoxFuture<'a, Result<(String, String), String>> {
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
        ) -> BoxFuture<'a, Result<(String, String), String>> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if attempt == 0 {
                    Err("网络错误: 连接超时".to_string())
                } else {
                    Ok(("https://example.test/audio".to_string(), String::new()))
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
            result.unwrap().0,
            "https://example.test/audio",
            "第二次请求成功后应立即返回真实链接"
        );
        assert_eq!(provider.attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn kuwo_link_does_not_read_qq_credential_file() {
        let path = std::env::temp_dir().join(format!(
            "hotdownloader-invalid-qq-{:x}.json",
            rand::random::<u64>()
        ));
        tokio::fs::write(&path, "{").await.unwrap();
        let provider = PlatformDownloadLinkProvider::from_credentials_file(&path);

        // 非法酷我 ID 在请求前失败；若错误来自 QQ 文件，说明平台隔离失效。
        let error = provider
            .fetch(Platform::Kuwo, "invalid-id", "320.mp3")
            .await
            .unwrap_err();
        assert!(error.contains("无效的歌曲 ID"));
        tokio::fs::remove_file(path).await.unwrap();
    }
}
