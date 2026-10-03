//! 自定义脚本音源的宿主限额与固定错误文案（契约 §5）。
//!
//! 这些常量同时被引擎（设置 boa 运行时限额）与命令层（返回用户可见文案）使用，
//! 单独成文件是为了让限额值可被单元测试直接断言。

/// 单个脚本正文上限（字节）。契约 §1：脚本 ≤256KB。
pub const MAX_SCRIPT_BYTES: usize = 256 * 1024;

/// 用户可见错误消息的截断长度（字符）。契约 §5：截断 200 字。
pub const MAX_ERROR_CHARS: usize = 200;

/// 循环迭代上限。契约 §5：5,000,000。
pub const MAX_LOOP_ITERATIONS: u64 = 5_000_000;

/// 递归深度上限。契约 §5：400。
pub const MAX_RECURSION_DEPTH: usize = 400;

/// 单次调用允许的 HTTP 请求数。契约 §5：16 次/调用。
pub const MAX_HTTP_REQUESTS_PER_CALL: u32 = 16;

/// 单次调用墙钟上限（毫秒）。契约 §5：20 秒。
pub const MAX_WALL_CLOCK_MS: u64 = 20_000;

/// `sleep` 单次上限（毫秒）。契约 §2：上限 2000，超过按 2000。
pub const MAX_SLEEP_MS: u64 = 2000;

/// `search` 单页条数默认值。契约 §3：默认 20。
pub const DEFAULT_PAGE_LIMIT: u32 = 20;

/// `search` 单页条数上限。契约 §3：上限 50。
pub const MAX_PAGE_LIMIT: u32 = 50;

/// 音源名称最大长度（字符）。契约 §1：name ≤32 字。
pub const MAX_NAME_CHARS: usize = 32;

/// 歌曲线索缓存**总条数**上限（所有音源共享，供 `getUrl` 回传脚本原始对象）。
pub const MAX_SONG_CLUES_TOTAL: usize = 200;

/// 单条歌曲线索的原始 JSON 上限（字节），超出则只保留归一化字段。
pub const MAX_SONG_CLUE_BYTES: usize = 16 * 1024;

/// 超限统一文案。契约 §5。
pub const TIMEOUT_MESSAGE: &str = "音源脚本超时或被限制";

/// 缺少音源名称的安装校验文案。契约 §5。
pub const MISSING_NAME_MESSAGE: &str = "脚本缺少 source.name";

/// 缺少方法（search/getUrl）的安装校验文案。契约 §5。
pub const MISSING_METHOD_MESSAGE: &str = "脚本缺少 search 或 getUrl 方法";

/// 用户可见错误前缀。契约 §5。
pub const ERROR_PREFIX: &str = "音源脚本错误：";

/// 脚本正文超限文案。
pub fn script_too_large_message(bytes: usize) -> String {
    format!("脚本过大（{bytes} 字节），上限为 {MAX_SCRIPT_BYTES} 字节")
}

/// 按字符截断错误消息，最多返回 [`MAX_ERROR_CHARS`] 个字符。
pub fn truncate_error(message: &str) -> String {
    let mut chars = message.chars();
    let collected: String = chars.by_ref().take(MAX_ERROR_CHARS - 1).collect();
    if chars.next().is_some() {
        // 仍有剩余字符：用省略号占满最后一个字符位，保证总长不超上限。
        format!("{collected}…")
    } else {
        collected
    }
}

/// 把任意脚本错误包装成用户可见文案（含前缀与截断）。
pub fn script_error(message: &str) -> String {
    format!("{ERROR_PREFIX}{}", truncate_error(message.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_match_contract_values() {
        assert_eq!(MAX_SCRIPT_BYTES, 262_144);
        assert_eq!(MAX_ERROR_CHARS, 200);
        assert_eq!(MAX_LOOP_ITERATIONS, 5_000_000);
        assert_eq!(MAX_RECURSION_DEPTH, 400);
        assert_eq!(MAX_HTTP_REQUESTS_PER_CALL, 16);
        assert_eq!(MAX_WALL_CLOCK_MS, 20_000);
        assert_eq!(MAX_SLEEP_MS, 2_000);
        assert_eq!(DEFAULT_PAGE_LIMIT, 20);
        assert_eq!(MAX_PAGE_LIMIT, 50);
        assert_eq!(MAX_NAME_CHARS, 32);
    }

    #[test]
    fn error_messages_stay_stable() {
        assert_eq!(TIMEOUT_MESSAGE, "音源脚本超时或被限制");
        assert_eq!(MISSING_NAME_MESSAGE, "脚本缺少 source.name");
        assert_eq!(MISSING_METHOD_MESSAGE, "脚本缺少 search 或 getUrl 方法");
        assert_eq!(ERROR_PREFIX, "音源脚本错误：");
    }

    #[test]
    fn truncate_error_caps_length_in_chars() {
        let short = "短错误";
        assert_eq!(truncate_error(short), short);

        // ASCII：199 个字符以内原样返回，200 个字符开始出现省略号。
        let exactly = "a".repeat(MAX_ERROR_CHARS);
        assert_eq!(truncate_error(&exactly).chars().count(), MAX_ERROR_CHARS);
        assert_eq!(truncate_error(&exactly), exactly);

        let longer = "a".repeat(MAX_ERROR_CHARS + 50);
        let truncated = truncate_error(&longer);
        assert_eq!(truncated.chars().count(), MAX_ERROR_CHARS);
        assert!(truncated.ends_with('…'));

        // 多字节字符按字符计数，不会切断码点。
        let chinese = "错".repeat(500);
        let truncated = truncate_error(&chinese);
        assert_eq!(truncated.chars().count(), MAX_ERROR_CHARS);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn script_error_adds_prefix_and_truncates() {
        assert_eq!(script_error("boom"), "音源脚本错误：boom");
        let long = script_error(&"x".repeat(1000));
        assert!(long.starts_with(ERROR_PREFIX));
        assert_eq!(
            long.chars().count(),
            ERROR_PREFIX.chars().count() + MAX_ERROR_CHARS
        );
    }
}
