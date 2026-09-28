//! 歌手名称的显示分隔符设置。

use serde_json::Value;

/// 未配置时用于连接多个歌手名称的分隔符。
pub const DEFAULT_SEPARATOR: &str = "、";

/// 设置缺失、类型错误或为空字符串时使用默认分隔符。
pub fn separator(settings: &Value) -> &str {
    settings
        .get("artistSeparator")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::{separator, DEFAULT_SEPARATOR};
    use serde_json::json;

    #[test]
    fn uses_valid_setting_or_default() {
        assert_eq!(separator(&json!({ "artistSeparator": "/" })), "/");
        for settings in [
            json!({}),
            json!({ "artistSeparator": "" }),
            json!({ "artistSeparator": 1 }),
        ] {
            assert_eq!(separator(&settings), DEFAULT_SEPARATOR);
        }
    }
}
