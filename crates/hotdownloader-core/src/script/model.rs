//! 脚本音源的数据模型（契约 §3/§4/§6/§7）。
//!
//! 这里只放纯数据与固定文案，不含引擎与 IO，方便单元测试直接断言序列化形状。

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::limits::{script_error, MAX_NAME_CHARS};

/// 持久化记录：脚本正文 + 元信息 + 启用状态（store 键 `scriptSources`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSource {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub qualities: Vec<String>,
    pub script: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub installed_at: String,
    /// 安装时使用的脚本来源 URL（可选，仅作展示与追溯）。
    #[serde(default)]
    pub source_url: Option<String>,
}

impl ScriptSource {
    /// 脚本正文长度（字节）。契约 §7：`scriptLength` 以字节计。
    pub fn script_length(&self) -> usize {
        self.script.len()
    }

    /// 去掉正文后的前端条目。
    pub fn item(&self) -> ScriptSourceItem {
        ScriptSourceItem {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            qualities: self.qualities.clone(),
            enabled: self.enabled,
            installed_at: self.installed_at.clone(),
            script_length: self.script_length(),
        }
    }
}

/// 前端可见条目：不含脚本正文。契约 §7。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSourceItem {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub qualities: Vec<String>,
    pub enabled: bool,
    pub installed_at: String,
    pub script_length: usize,
}

/// 命令返回值：`list_script_sources` / 安装与删除后的完整快照。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSourceState {
    pub sources: Vec<ScriptSourceItem>,
}

/// 脚本返回的歌曲（宿主归一化后的字段集）。契约 §3。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptSong {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    /// 时长（秒）；用于避免下错歌。
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub qualities: Vec<String>,
    /// 脚本返回的原始对象（JSON 文本），`getUrl` 时原样回传。
    #[serde(default)]
    pub raw: Option<String>,
}

impl ScriptSong {
    /// `getUrl` 的入参：优先回传脚本原始对象，缺失时用归一化字段重建。
    pub fn to_script_json(&self) -> String {
        if let Some(raw) = self.raw.as_deref() {
            if !raw.trim().is_empty() {
                return raw.to_string();
            }
        }
        let qualities = serde_json::Value::Array(
            self.qualities
                .iter()
                .map(|quality| serde_json::Value::String(quality.clone()))
                .collect(),
        );
        serde_json::json!({
            "id": self.id,
            "title": self.title,
            "artist": self.artist,
            "album": self.album,
            "duration": self.duration,
            "cover": self.cover,
            "qualities": qualities,
        })
        .to_string()
    }
}

/// `getUrl` 的返回值（已归一化）。契约 §4。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptUrl {
    pub url: String,
    /// 脚本自报的音质；缺省表示按请求音质认定。
    #[serde(default)]
    pub quality: Option<String>,
    /// 下载该直链需要附加的请求头（顺序保留，允许重名）。
    #[serde(default, deserialize_with = "deserialize_headers")]
    pub headers: Vec<(String, String)>,
}

/// 脚本侧 `headers` 是对象（`{ "Referer": "…" }`）或缺省 `null`，这里归一化成有序键值对。
fn deserialize_headers<'de, D>(deserializer: D) -> Result<Vec<(String, String)>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<serde_json::Map<String, serde_json::Value>>::deserialize(deserializer)?;
    Ok(raw
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(name, value)| value.as_str().map(|text| (name, text.to_string())))
        .collect())
}

impl ScriptUrl {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// `test_script_source` 的返回值。契约 §7。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTestReport {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample: Option<ScriptSong>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ScriptTestReport {
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            name: None,
            qualities: None,
            sample: None,
            error: Some(message.into()),
        }
    }
}

/// 脚本调用错误。
///
/// `transient` 表示网络类错误：下载链路会按既有重试语义重试，
/// 确定性错误（脚本抛错、契约违反）直接上报给用户。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptCallError {
    pub message: String,
    pub transient: bool,
}

/// 允许 `run_blocking` 用 `?` 把线程异常文案转成脚本错误。
impl From<String> for ScriptCallError {
    fn from(message: String) -> Self {
        Self::from_message(message)
    }
}

impl ScriptCallError {
    pub fn deterministic(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transient: false,
        }
    }

    pub fn transient(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transient: true,
        }
    }

    pub fn from_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if crate::download::link::is_retryable_link_error(&message) {
            Self::transient(message)
        } else {
            Self::deterministic(message)
        }
    }

    /// 用户可见文案：确定性错误加契约前缀并截断，网络类错误保持原前缀以便重试判定。
    pub fn user_message(&self) -> String {
        if self.transient {
            self.message.clone()
        } else {
            script_error(&self.message)
        }
    }
}

impl fmt::Display for ScriptCallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.user_message())
    }
}

/// 安装时对 `source.name` 的校验（含长度上限）。契约 §1。
pub fn normalize_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(super::limits::MISSING_NAME_MESSAGE.to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("音源名称过长，上限为 {MAX_NAME_CHARS} 字"));
    }
    Ok(name.to_string())
}

/// 当前时间的 ISO8601（UTC，秒精度）。
pub fn iso8601_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    iso8601_from_unix(seconds)
}

/// Unix 秒 → ISO8601（UTC）。用 civil-from-days 算法手写，避免引入时间库。
pub fn iso8601_from_unix(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let remaining = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        remaining / 3_600,
        (remaining % 3_600) / 60,
        remaining % 60
    )
}

/// Howard Hinnant 的 civil_from_days：把 1970-01-01 起的天数换成公历日期。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // [0, 399]
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let month_prime = (5 * day_of_year + 2) / 153; // [0, 11]
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32; // [1, 31]
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_formats_known_instants() {
        assert_eq!(iso8601_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601_from_unix(1_700_000_000), "2023-11-14T22:13:20Z");
        // 闰年 2 月 29 日
        assert_eq!(iso8601_from_unix(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn source_item_hides_script_body() {
        let source = ScriptSource {
            id: 7,
            name: "示例".into(),
            description: Some("说明".into()),
            qualities: vec!["320kmp3".into()],
            script: "var source = {};".into(),
            enabled: true,
            installed_at: "2026-01-01T00:00:00Z".into(),
            source_url: None,
        };
        let item = source.item();
        assert_eq!(item.script_length, source.script.len());
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["installedAt"], "2026-01-01T00:00:00Z");
        assert_eq!(json["scriptLength"], source.script.len());
        assert!(json.get("script").is_none());
    }

    #[test]
    fn transient_classification_follows_retry_prefixes() {
        let retryable = ScriptCallError::from_message("网络错误: 连接超时");
        assert!(retryable.transient);
        assert_eq!(retryable.user_message(), "网络错误: 连接超时");

        let deterministic = ScriptCallError::from_message("找不到该歌曲");
        assert!(!deterministic.transient);
        assert_eq!(deterministic.user_message(), "音源脚本错误：找不到该歌曲");
    }

    #[test]
    fn name_validation_uses_contract_message() {
        assert_eq!(
            normalize_name("   ").unwrap_err(),
            super::super::limits::MISSING_NAME_MESSAGE
        );
        assert_eq!(normalize_name(" 测试音源 ").unwrap(), "测试音源");
        assert!(normalize_name(&"长".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn song_json_falls_back_to_normalized_fields() {
        let song = ScriptSong {
            id: "abc".into(),
            title: "标题".into(),
            artist: "歌手".into(),
            album: Some("专辑".into()),
            duration: Some(200),
            cover: None,
            qualities: vec!["128kmp3".into()],
            raw: None,
        };
        let value: serde_json::Value = serde_json::from_str(&song.to_script_json()).unwrap();
        assert_eq!(value["id"], "abc");
        assert_eq!(value["duration"], 200);

        let with_raw = ScriptSong {
            raw: Some("{\"id\":\"abc\",\"extra\":1}".into()),
            ..song
        };
        let value: serde_json::Value = serde_json::from_str(&with_raw.to_script_json()).unwrap();
        assert_eq!(value["extra"], 1);
    }

    #[test]
    fn url_result_accepts_header_object_and_null() {
        let with_headers: ScriptUrl = serde_json::from_str(
            r#"{"url":"https://cdn.example.com/a.mp3","quality":"320kmp3","headers":{"Referer":"https://y.example.com","Origin":"https://y.example.com"}}"#,
        )
        .unwrap();
        assert_eq!(with_headers.headers.len(), 2);
        assert_eq!(
            with_headers.header("referer"),
            Some("https://y.example.com")
        );

        let without_headers: ScriptUrl =
            serde_json::from_str(r#"{"url":"https://cdn.example.com/a.mp3","headers":null}"#)
                .unwrap();
        assert!(without_headers.headers.is_empty());
        assert!(without_headers.quality.is_none());
    }
}
