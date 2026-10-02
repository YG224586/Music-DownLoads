use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// 支持的音乐平台标识；具体 API 实现由运行时的 worker 提供。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    /// QQ 音乐
    #[serde(rename = "qqmusic")]
    QqMusic,
    /// 酷我音乐
    #[serde(rename = "kuwo")]
    Kuwo,
}

impl FromStr for Platform {
    type Err = String;

    /// 从前端或持久化记录中的平台字符串解析。
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "qqmusic" => Ok(Self::QqMusic),
            "kuwo" => Ok(Self::Kuwo),
            _ => Err(format!("不支持的平台: {value}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Platform;

    #[test]
    fn platform_strings_keep_existing_error_contract() {
        assert_eq!("qqmusic".parse::<Platform>().unwrap(), Platform::QqMusic);
        assert_eq!("kuwo".parse::<Platform>().unwrap(), Platform::Kuwo);
        assert_eq!(
            "other".parse::<Platform>().unwrap_err(),
            "不支持的平台: other"
        );
    }
}
