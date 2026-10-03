use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 自定义脚本音源的稳定标识。
///
/// 保持 `Copy`：`Platform` 会被大量按值拷贝（任务上下文、worker、下载链路），
/// 一旦这里不再是 `Copy`，所有 `ctx.platform` 拷贝点都要跟着改为克隆。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScriptId(pub u32);

impl ScriptId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ScriptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// 平台字符串里自定义脚本音源的前缀，例如 `script:12`。
pub const SCRIPT_PLATFORM_PREFIX: &str = "script:";

/// 支持的音乐平台标识；具体 API 实现由运行时的 worker 提供。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// QQ 音乐
    QqMusic,
    /// 酷我音乐
    Kuwo,
    /// 酷狗音乐
    Kugou,
    /// 网易云音乐
    Netease,
    /// 哔哩哔哩音频
    Bilibili,
    /// 咪咕音乐
    Migu,
    /// 用户自定义脚本音源
    Script(ScriptId),
}

impl Platform {
    /// 是否为用户自定义脚本音源。
    pub const fn is_script(self) -> bool {
        matches!(self, Self::Script(_))
    }

    /// 脚本音源返回的 id；内置平台返回 `None`。
    pub const fn script_id(self) -> Option<ScriptId> {
        match self {
            Self::Script(id) => Some(id),
            _ => None,
        }
    }

    /// 是否为内置平台（QQ 音乐、酷我、酷狗、网易云、哔哩哔哩、咪咕）。
    pub const fn is_builtin(self) -> bool {
        !self.is_script()
    }

    /// 持久化与前端使用的稳定字符串。
    pub fn as_str(self) -> String {
        self.to_string()
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QqMusic => formatter.write_str("qqmusic"),
            Self::Kuwo => formatter.write_str("kuwo"),
            Self::Kugou => formatter.write_str("kugou"),
            Self::Netease => formatter.write_str("netease"),
            Self::Bilibili => formatter.write_str("bilibili"),
            Self::Migu => formatter.write_str("migu"),
            Self::Script(id) => write!(formatter, "{SCRIPT_PLATFORM_PREFIX}{id}"),
        }
    }
}

impl FromStr for Platform {
    type Err = String;

    /// 从前端或持久化记录中的平台字符串解析。
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "qqmusic" => Ok(Self::QqMusic),
            "kuwo" => Ok(Self::Kuwo),
            "kugou" => Ok(Self::Kugou),
            "netease" => Ok(Self::Netease),
            "bilibili" => Ok(Self::Bilibili),
            "migu" => Ok(Self::Migu),
            _ => match value.strip_prefix(SCRIPT_PLATFORM_PREFIX) {
                Some(raw_id) => raw_id
                    .parse::<u32>()
                    .map(|id| Self::Script(ScriptId(id)))
                    .map_err(|_| format!("不支持的平台: {value}")),
                None => Err(format!("不支持的平台: {value}")),
            },
        }
    }
}

// 手写 serde：需要把 `Script(12)` 序列化成字符串 `"script:12"`，
// derive 的 newtype 变体只会产出 `{"script":12}`，与契约不符。
impl Serialize for Platform {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Platform {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse::<Platform>().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{Platform, ScriptId};
    use std::str::FromStr;

    #[test]
    fn platform_strings_keep_existing_error_contract() {
        assert_eq!("qqmusic".parse::<Platform>().unwrap(), Platform::QqMusic);
        assert_eq!("kuwo".parse::<Platform>().unwrap(), Platform::Kuwo);
        assert_eq!("kugou".parse::<Platform>().unwrap(), Platform::Kugou);
        assert_eq!("netease".parse::<Platform>().unwrap(), Platform::Netease);
        assert_eq!("bilibili".parse::<Platform>().unwrap(), Platform::Bilibili);
        assert_eq!("migu".parse::<Platform>().unwrap(), Platform::Migu);
        assert_eq!(
            "other".parse::<Platform>().unwrap_err(),
            "不支持的平台: other"
        );
    }

    #[test]
    fn every_builtin_platform_round_trips_through_its_string() {
        for platform in [
            Platform::QqMusic,
            Platform::Kuwo,
            Platform::Kugou,
            Platform::Netease,
            Platform::Bilibili,
            Platform::Migu,
        ] {
            let raw = platform.to_string();
            assert_eq!(Platform::from_str(&raw).unwrap(), platform, "{raw}");
            assert!(platform.is_builtin());
            assert!(!platform.is_script());
            assert_eq!(platform.script_id(), None);
        }
    }

    #[test]
    fn script_platform_display_and_from_str_round_trip() {
        let platform = Platform::Script(ScriptId::new(12));
        assert_eq!(platform.to_string(), "script:12");
        assert_eq!(Platform::from_str("script:12").unwrap(), platform);
        assert_eq!(platform.script_id(), Some(ScriptId::new(12)));
        assert!(platform.is_script());
        assert!(!Platform::QqMusic.is_script());
    }

    #[test]
    fn script_platform_rejects_invalid_ids_with_legacy_message() {
        assert_eq!(
            "script:".parse::<Platform>().unwrap_err(),
            "不支持的平台: script:"
        );
        assert_eq!(
            "script:abc".parse::<Platform>().unwrap_err(),
            "不支持的平台: script:abc"
        );
        assert_eq!(
            "script:-1".parse::<Platform>().unwrap_err(),
            "不支持的平台: script:-1"
        );
        // 溢出 u32 的 id 同样走旧文案，避免把 serde 内部错误泄露给前端。
        assert_eq!(
            "script:4294967296".parse::<Platform>().unwrap_err(),
            "不支持的平台: script:4294967296"
        );
    }

    #[test]
    fn platform_serializes_to_contract_strings() {
        assert_eq!(
            serde_json::to_string(&Platform::QqMusic).unwrap(),
            "\"qqmusic\""
        );
        assert_eq!(serde_json::to_string(&Platform::Kuwo).unwrap(), "\"kuwo\"");
        assert_eq!(
            serde_json::to_string(&Platform::Kugou).unwrap(),
            "\"kugou\""
        );
        assert_eq!(
            serde_json::to_string(&Platform::Netease).unwrap(),
            "\"netease\""
        );
        assert_eq!(
            serde_json::to_string(&Platform::Bilibili).unwrap(),
            "\"bilibili\""
        );
        assert_eq!(serde_json::to_string(&Platform::Migu).unwrap(), "\"migu\"");
        assert_eq!(
            serde_json::to_string(&Platform::Script(ScriptId::new(12))).unwrap(),
            "\"script:12\""
        );

        let parsed: Platform = serde_json::from_str("\"script:12\"").unwrap();
        assert_eq!(parsed, Platform::Script(ScriptId::new(12)));
        assert!(serde_json::from_str::<Platform>("\"script:abc\"").is_err());
    }
}
