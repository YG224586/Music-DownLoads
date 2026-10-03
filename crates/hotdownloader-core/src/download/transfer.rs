use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_LENGTH, CONTENT_RANGE, RANGE};

use crate::download::decryption::{self, DecryptContext};
use crate::download::engine::TaskController;
use crate::download::ports::DownloadProgressSink;

/// 自定义请求头的条数上限（来自不可信的音源脚本）。
pub const MAX_CUSTOM_HEADERS: usize = 16;
/// 自定义请求头「名称 + 取值」总字节数上限。
pub const MAX_CUSTOM_HEADER_BYTES: usize = 4096;

/// 禁止脚本覆盖的请求头：传输层或客户端自己管理这些语义。
/// 覆盖 `Range` 会破坏断点续传，覆盖 `Host`/`Content-Length`/`Connection`/
/// `Transfer-Encoding` 会破坏请求本身。
fn is_forbidden_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host" | "content-length" | "connection" | "transfer-encoding" | "range"
    )
}

/// 把不可信脚本给出的请求头收敛成可安全附加到单次媒体请求上的集合。
///
/// 规则（超限一律丢弃并告警，**只记名称不记取值**，取值可能含 Cookie）：
/// - 最多 [`MAX_CUSTOM_HEADERS`] 条，「名称 + 取值」总字节数最多 [`MAX_CUSTOM_HEADER_BYTES`]，
///   达到上限后其余请求头全部丢弃；
/// - 命中 [`is_forbidden_header`] 或取值含 CR/LF 的请求头丢弃（防请求头注入/续传被破坏）；
/// - 名称必须是合法 HTTP token、取值必须是合法字段值（含控制字符即丢弃）。
///
/// 这些请求头只作用于这一条下载请求，不会写入全局客户端，也不会持久化。
pub fn sanitize_custom_headers(
    headers: &[(String, String)],
    task_id: &str,
) -> Vec<(HeaderName, HeaderValue)> {
    let mut accepted: Vec<(HeaderName, HeaderValue)> = Vec::new();
    let mut bytes = 0usize;
    let mut dropped = 0usize;
    // 字节预算用尽后不再接受任何请求头（与「达到上限后其余请求头全部丢弃」一致），
    // 避免超限请求头之后的小请求头又把总预算撑回去。
    let mut budget_exhausted = false;

    for (name, value) in headers {
        if accepted.len() >= MAX_CUSTOM_HEADERS || budget_exhausted {
            dropped += 1;
            continue;
        }
        if bytes + name.len() + value.len() > MAX_CUSTOM_HEADER_BYTES {
            budget_exhausted = true;
            dropped += 1;
            continue;
        }
        if is_forbidden_header(name) || value.contains(['\r', '\n']) {
            log::warn!("任务 {task_id} 丢弃自定义请求头 {name}：禁止覆盖该请求头或取值含换行");
            dropped += 1;
            continue;
        }
        let parsed = HeaderName::from_bytes(name.as_bytes())
            .ok()
            .zip(HeaderValue::from_str(value).ok());
        let Some((parsed_name, parsed_value)) = parsed else {
            log::warn!("任务 {task_id} 丢弃自定义请求头 {name}：名称或取值不是合法 HTTP 字段");
            dropped += 1;
            continue;
        };
        bytes += name.len() + value.len();
        accepted.push((parsed_name, parsed_value));
    }

    if dropped > 0 {
        log::warn!(
            "任务 {task_id} 共丢弃 {dropped} 条自定义请求头（上限 {MAX_CUSTOM_HEADERS} 条 / {MAX_CUSTOM_HEADER_BYTES} 字节，禁止覆盖 host/content-length/connection/transfer-encoding/range）"
        );
    }

    accepted
}

/// 对比内置默认 `Referer` 与脚本自带请求头的取舍：脚本自带时以脚本为准，
/// 避免同一个请求头出现两次（部分 CDN 会因此拒绝请求）。
fn needs_default_referer(headers: &[(HeaderName, HeaderValue)]) -> bool {
    !headers
        .iter()
        .any(|(name, _)| name.as_str().eq_ignore_ascii_case("referer"))
}

/// 下载专用客户端不设置总超时，大文件只限制连接和单次读取等待时间。
static DOWNLOAD_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn download_client() -> &'static reqwest::Client {
    DOWNLOAD_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("HotDownloader/1.0")
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(300))
            .build()
            .expect("Failed to create download HTTP client")
    })
}

/// 使用当前已写入字节数请求 HTTP Range；临时请求错误最多尝试三次。
/// 返回原始响应，以便文件适配器处理普通路径或 Android SAF 文件。
///
/// `headers` 是取链阶段带来的**不可信**自定义请求头（目前来自音源脚本）：
/// 先经 [`sanitize_custom_headers`] 收敛，再只附加到这一条媒体请求上；
/// 传入空切片时请求与改动前逐字节一致。
pub async fn request_download_response(
    url: &str,
    downloaded: u64,
    task_id: &str,
    headers: &[(String, String)],
) -> Result<reqwest::Response, reqwest::Error> {
    // 校验只做一次：结果在重试之间保持不变。
    let custom_headers = sanitize_custom_headers(headers, task_id);
    let mut attempt = 0;
    loop {
        let mut request = download_client().get(url);
        // 平台默认 Referer 保持原行为；脚本自带 Referer 时以脚本为准，避免出现重复请求头。
        if needs_default_referer(&custom_headers) {
            request = request.header("Referer", "https://y.qq.com");
        }
        for (name, value) in &custom_headers {
            request = request.header(name.clone(), value.clone());
        }
        if downloaded > 0 {
            request = request.header(RANGE, format!("bytes={downloaded}-"));
        }

        match request.send().await {
            Ok(response) => return Ok(response),
            Err(error) => {
                attempt += 1;
                log::warn!("任务 {task_id} 下载请求失败 (尝试 {attempt}/3): {error}");
                if is_retryable_network_error(&error) && attempt < 3 {
                    // 两次等待依次为 1 秒、2 秒，第三次失败直接交给 worker 上报。
                    tokio::time::sleep(Duration::from_secs(1 << (attempt - 1))).await;
                } else {
                    return Err(error);
                }
            }
        }
    }
}

/// reqwest 错误分类留在传输核心，平台接口返回的字符串错误另由链接模块判断。
pub fn is_retryable_network_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || (error.is_request() && !error.is_body())
}

/// worker 根据响应决定是否重新打开文件、继续流写入或上报错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseAction {
    Stream { total: u64 },
    RestartFromBeginning { total: u64 },
    InvalidRange,
    AlreadyComplete,
    LinkExpired,
    HttpError(u16),
}

/// 一次 HTTP 响应读取结束后，worker 据此决定完成、等待恢复或重新获取链接。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOutcome {
    Completed,
    Paused,
    Canceled,
    Retry,
    Failed,
}

/// 单次响应流处理期间不变的任务信息与输出端口。
pub struct StreamWriteContext<'a> {
    pub total: u64,
    pub decrypt_context: &'a DecryptContext,
    pub controller: &'a TaskController,
    pub progress_sink: &'a dyn DownloadProgressSink,
    pub task_id: &'a str,
}

/// 把响应流按文件绝对偏移解密并写入目标。文件可以是普通文件，也可以是 SAF 适配器打开的文件。
/// 文件的创建和重新打开仍由上层负责，因为 Android SAF 需要平台授权及 URI。
pub async fn write_response_stream<W: Write + Send>(
    response: reqwest::Response,
    writer: &mut W,
    downloaded: &mut u64,
    stream_retries: &mut u32,
    context: StreamWriteContext<'_>,
) -> StreamOutcome {
    let StreamWriteContext {
        total,
        decrypt_context,
        controller,
        progress_sink,
        task_id,
    } = context;
    let mut stream = response.bytes_stream();
    let mut last_report = Instant::now();
    let mut last_downloaded = *downloaded;
    let mut speed_samples = VecDeque::with_capacity(5);

    loop {
        if controller.cancel_token.is_cancelled() {
            return StreamOutcome::Canceled;
        }
        if controller.pause_flag.load(Ordering::SeqCst) {
            return StreamOutcome::Paused;
        }

        // 读取网络数据期间也响应取消，不让移除任务长期等待读取超时。
        let chunk_result = tokio::select! {
            _ = controller.cancel_token.cancelled() => return StreamOutcome::Canceled,
            chunk = stream.next() => chunk,
        };
        let chunk = match chunk_result {
            Some(Ok(bytes)) => bytes,
            Some(Err(error)) => {
                log::error!("任务 {task_id} 读取流错误: {error}");
                return retry_or_fail(
                    stream_retries,
                    progress_sink,
                    task_id,
                    &format!("读取流错误: {error}"),
                );
            }
            None => {
                // 正常 EOF 只有在文件大小未知或达到预期大小时才算完成。
                // 已知大小却提前结束时重新请求 Range，防止任务永远留在下载中。
                if total > 0 && *downloaded < total {
                    return retry_or_fail(
                        stream_retries,
                        progress_sink,
                        task_id,
                        "下载流提前结束，请重试",
                    );
                }
                return flush_completed(writer, progress_sink, task_id);
            }
        };

        let mut chunk_data = chunk.to_vec();
        let chunk_len = chunk_data.len() as u64;
        decryption::decrypt_chunk(decrypt_context, &mut chunk_data, *downloaded);
        if let Err(error) = writer.write_all(&chunk_data) {
            log::error!("写入文件错误: {error}");
            progress_sink.error(task_id, &format!("写入文件失败: {error}"));
            return StreamOutcome::Failed;
        }
        *downloaded += chunk_len;

        let now = Instant::now();
        let elapsed = now - last_report;
        if elapsed >= Duration::from_millis(500) {
            let instant_speed =
                ((*downloaded - last_downloaded) as f64 / elapsed.as_secs_f64()) as u64;
            speed_samples.push_back(instant_speed);
            if speed_samples.len() > 5 {
                speed_samples.pop_front();
            }
            let average_speed = speed_samples.iter().sum::<u64>() / speed_samples.len() as u64;
            progress_sink.progress(task_id, *downloaded, total, average_speed);
            last_report = now;
            last_downloaded = *downloaded;
        }

        if total > 0 && *downloaded >= total {
            return flush_completed(writer, progress_sink, task_id);
        }
    }
}

/// 流错误累计最多重试两次；第三次必须进入明确的错误状态。
fn retry_or_fail(
    stream_retries: &mut u32,
    progress_sink: &dyn DownloadProgressSink,
    task_id: &str,
    message: &str,
) -> StreamOutcome {
    if *stream_retries < 2 {
        *stream_retries += 1;
        StreamOutcome::Retry
    } else {
        progress_sink.error(task_id, message);
        StreamOutcome::Failed
    }
}

/// 只有缓冲区真正写入成功后，才能将传输视为完成。
fn flush_completed<W: Write>(
    writer: &mut W,
    progress_sink: &dyn DownloadProgressSink,
    task_id: &str,
) -> StreamOutcome {
    if let Err(error) = writer.flush() {
        log::error!("刷新文件缓冲区失败: {error}");
        progress_sink.error(task_id, &format!("刷新文件缓冲区失败: {error}"));
        StreamOutcome::Failed
    } else {
        StreamOutcome::Completed
    }
}

/// 从真实 HTTP 响应提取状态与头部，判断续传是否安全。
pub fn classify_http_response(
    response: &reqwest::Response,
    downloaded: u64,
    fallback_size: u64,
) -> ResponseAction {
    let content_range = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok());
    let content_length = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok());
    classify_response(
        response.status().as_u16(),
        content_range,
        content_length,
        downloaded,
        fallback_size,
    )
}

/// 与运行时无关的 HTTP 响应规则。原有 416 行为视为文件已下载完成。
pub fn classify_response(
    status: u16,
    content_range: Option<&str>,
    content_length: Option<&str>,
    downloaded: u64,
    fallback_size: u64,
) -> ResponseAction {
    // Content-Range 的末尾为整个文件大小；缺失或无效时退回 Content-Length/任务快照。
    let reported_size = if let Some(range) = content_range {
        range
            .rsplit('/')
            .next()
            .and_then(|value| value.parse().ok())
    } else {
        content_length.and_then(|value| value.parse().ok())
    };
    let total = reported_size
        .filter(|size| *size > 0)
        .unwrap_or(fallback_size);

    // CDN 忽略 Range 时返回完整 200，继续追加会把两份音频拼在一起。
    if downloaded > 0 && status == 200 {
        return ResponseAction::RestartFromBeginning { total };
    }

    if downloaded > 0 && status == 206 {
        let range_start = content_range
            .and_then(|value| value.strip_prefix("bytes "))
            .and_then(|value| value.split('-').next())
            .and_then(|value| value.parse::<u64>().ok());
        if range_start != Some(downloaded) {
            return ResponseAction::InvalidRange;
        }
    }

    if status == 416 {
        return ResponseAction::AlreadyComplete;
    }
    if matches!(status, 403 | 404 | 410) {
        return ResponseAction::LinkExpired;
    }
    if (400..600).contains(&status) {
        return ResponseAction::HttpError(status);
    }

    ResponseAction::Stream { total }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{
        classify_response, needs_default_referer, retry_or_fail, sanitize_custom_headers,
        ResponseAction, StreamOutcome, MAX_CUSTOM_HEADERS, MAX_CUSTOM_HEADER_BYTES,
    };
    use crate::download::ports::DownloadProgressSink;

    fn pairs(values: &[(&str, &str)]) -> Vec<(String, String)> {
        values
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    #[derive(Default)]
    struct RecordingProgressSink {
        errors: Mutex<Vec<String>>,
    }

    impl DownloadProgressSink for RecordingProgressSink {
        fn progress(&self, _task_id: &str, _downloaded: u64, _total: u64, _speed: u64) {}
        fn file_complete(&self, _task_id: &str) {}
        fn completed(&self, _task_id: &str, _final_path: &str, _saf_folder_uri: Option<String>) {}
        fn error(&self, _task_id: &str, message: &str) {
            self.errors.lock().unwrap().push(message.to_string());
        }
        fn link_expired(&self, _task_id: &str, _current_offset: u64) {}
        fn metadata_error(&self, _task_id: &str, _message: &str) {}
    }

    #[test]
    fn ignored_range_requires_reopening_file_from_zero() {
        assert_eq!(
            classify_response(200, None, Some("100"), 30, 100),
            ResponseAction::RestartFromBeginning { total: 100 }
        );
    }

    #[test]
    fn partial_response_must_start_at_requested_offset() {
        assert_eq!(
            classify_response(206, Some("bytes 10-99/100"), None, 30, 100),
            ResponseAction::InvalidRange
        );
        assert_eq!(
            classify_response(206, Some("bytes 30-99/100"), Some("70"), 30, 70),
            ResponseAction::Stream { total: 100 }
        );
    }

    #[test]
    fn expired_link_and_unknown_size_follow_existing_contract() {
        assert_eq!(
            classify_response(403, None, None, 12, 100),
            ResponseAction::LinkExpired
        );
        assert_eq!(
            classify_response(416, None, None, 100, 100),
            ResponseAction::AlreadyComplete
        );
        assert_eq!(
            classify_response(200, None, None, 0, 500),
            ResponseAction::Stream { total: 500 }
        );
    }

    #[test]
    fn repeated_stream_failure_reports_terminal_error() {
        let sink = RecordingProgressSink::default();
        let mut retries = 0;

        assert_eq!(
            retry_or_fail(&mut retries, &sink, "task", "断流"),
            StreamOutcome::Retry
        );
        assert_eq!(
            retry_or_fail(&mut retries, &sink, "task", "断流"),
            StreamOutcome::Retry
        );
        assert_eq!(
            retry_or_fail(&mut retries, &sink, "task", "断流"),
            StreamOutcome::Failed
        );
        assert_eq!(retries, 2);
        assert_eq!(sink.errors.lock().unwrap().as_slice(), ["断流"]);
    }

    #[test]
    fn empty_custom_headers_keep_the_default_request_shape() {
        let accepted = sanitize_custom_headers(&[], "task");
        assert!(accepted.is_empty());
        // 没有任何自定义头时仍然附加内置 Referer，与改动前一致。
        assert!(needs_default_referer(&accepted));
    }

    #[test]
    fn custom_headers_are_capped_at_sixteen() {
        let headers: Vec<(String, String)> = (0..20)
            .map(|index| (format!("x-test-{index}"), "1".to_string()))
            .collect();
        let accepted = sanitize_custom_headers(&headers, "task");

        assert_eq!(accepted.len(), MAX_CUSTOM_HEADERS);
        assert_eq!(accepted[0].0.as_str(), "x-test-0");
        assert_eq!(
            accepted[MAX_CUSTOM_HEADERS - 1].0.as_str(),
            "x-test-15",
            "超出的请求头必须按顺序被截断"
        );
    }

    #[test]
    fn headers_beyond_the_byte_budget_are_dropped() {
        let oversized = "a".repeat(MAX_CUSTOM_HEADER_BYTES);
        let headers = pairs(&[("referer", "https://y.qq.com"), ("cookie", &oversized)]);
        let accepted = sanitize_custom_headers(&headers, "task");

        assert_eq!(accepted.len(), 1, "单条超字节上限的请求头必须整条丢弃");
        assert_eq!(accepted[0].0.as_str(), "referer");

        // 累计字节数同样受限：第二条撑爆剩余预算，它与其后的请求头一起被丢弃。
        // 取值长度必须大于「上限 − 第一条的 name+value」，否则根本触发不了累计上限。
        let big = "b".repeat(MAX_CUSTOM_HEADER_BYTES - 8);
        let headers = vec![
            ("referer".to_string(), "https://y.qq.com".to_string()),
            ("cookie".to_string(), big),
            ("x-extra".to_string(), "1".to_string()),
        ];
        let accepted = sanitize_custom_headers(&headers, "task");
        let names: Vec<&str> = accepted.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["referer"]);
    }

    #[test]
    fn forbidden_and_injected_headers_are_dropped() {
        let headers = pairs(&[
            ("Host", "evil.test"),
            ("range", "bytes=0-1"),
            ("CONTENT-LENGTH", "10"),
            ("Connection", "keep-alive"),
            ("Transfer-Encoding", "chunked"),
            ("X-Bad", "line1\r\nHost: evil.test"),
            ("X-Good", "ok"),
        ]);
        let accepted = sanitize_custom_headers(&headers, "task");

        assert_eq!(accepted.len(), 1, "禁止名单与换行注入都必须被丢弃");
        assert_eq!(accepted[0].0.as_str(), "x-good");
        assert_eq!(accepted[0].1.to_str().unwrap(), "ok");
    }

    #[test]
    fn invalid_names_and_control_characters_are_dropped() {
        let headers = pairs(&[("X Space", "1"), ("X-Ctrl", "a\u{1}b"), ("X-Ok", "fine")]);
        let accepted = sanitize_custom_headers(&headers, "task");

        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].0.as_str(), "x-ok");
    }

    #[test]
    fn script_referer_replaces_the_builtin_default() {
        let without = sanitize_custom_headers(&pairs(&[("X-A", "1")]), "task");
        assert!(needs_default_referer(&without));

        let with = sanitize_custom_headers(&pairs(&[("Referer", "https://a.test")]), "task");
        assert!(
            !needs_default_referer(&with),
            "脚本自带 Referer 时不得再附加内置 Referer"
        );
        assert_eq!(with[0].1.to_str().unwrap(), "https://a.test");
    }
}
