use serde::{Deserialize, Serialize};

/// 歌曲 ID 归一化：能解析成数字就用原值，否则用 FNV-1a 稳定哈希。
///
/// 搜索结果里的 `id` 形状按平台而异：QQ 音乐/酷我是纯数字，网易云/咪咕是数字字符串，
/// 哔哩哔哩是 BV 号。任务契约定的是 `u64`（`song_id` 只服务歌词/封面等次要查找，
/// 取链一律走 `mid`），所以字符串 ID 必须在这里统一折算，否则整条建任务请求
/// 会因 `invalid type: string …, expected u64` 反序列化失败。
pub fn song_id_to_u64(song_id: &str) -> u64 {
    if let Ok(value) = song_id.trim().parse::<u64>() {
        return value;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in song_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// 宽容反序列化歌曲 ID：数字、数字字符串、其它字符串（稳定哈希）与 null 都接受。
fn deserialize_song_id<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct SongIdVisitor;

    impl<'de> serde::de::Visitor<'de> for SongIdVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("数字或字符串形式的歌曲 ID")
        }

        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }

        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<u64, E> {
            u64::try_from(value).map_err(|_| E::custom("歌曲 ID 不能为负数"))
        }

        fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<u64, E> {
            if value.is_finite() && (0.0..=u64::MAX as f64).contains(&value) {
                Ok(value as u64)
            } else {
                Err(E::custom("歌曲 ID 不是有效的数字"))
            }
        }

        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<u64, E> {
            Ok(song_id_to_u64(value))
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<u64, E> {
            Ok(0)
        }
    }

    deserializer.deserialize_any(SongIdVisitor)
}

/// 歌曲的可下载品质；文件名和大小与品质绑定，降级时需要一并更新。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityItem {
    pub quality: String,
    pub filename: String,
    pub size: u64,
}

/// 前端提交的歌曲元数据。具体下载行为由 Rust 侧根据设置和任务规则决定。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongInput {
    pub platform: String,
    /// 归一化后的数字 ID；前端可能回传字符串（见 [`song_id_to_u64`]）。
    #[serde(deserialize_with = "deserialize_song_id")]
    pub id: u64,
    pub mid: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub media_mid: String,
    pub qualities: Vec<QualityItem>,
}

// 引入平台字段前仅支持 QQ 音乐，历史任务缺失该字段时沿用原平台。
fn legacy_task_platform() -> String {
    "qqmusic".to_owned()
}

/// 持久化任务契约：字段名沿用前端已有的 camelCase 数据，兼容旧任务记录。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    #[serde(default = "legacy_task_platform")]
    pub platform: String,
    pub song_id: u64,
    pub song_mid: String,
    pub song_title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub media_mid: String,
    pub filename: String,
    pub quality: String,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_msg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    pub file_size: u64,
    pub downloaded: u64,
    pub retry_count: u32,
    pub added_at: u64,
    // 旧记录可能没有品质列表；这类任务重试时无法自动选择降级品质。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_qualities: Option<Vec<QualityItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<u64>,
    // 目标路径在建任务时确定，保证排队任务也能参与重名冲突检查。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Waiting,
    Downloading,
    Paused,
    Processing,
    Completed,
    Error,
    /// 进程重启后等待用户恢复的任务；恢复不消耗下载错误重试次数。
    Interrupted,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DuplicateAction {
    Overwrite,
    Rename,
    Cancel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub song: SongInput,
    pub desired_quality: String,
    pub duplicate_action: Option<DuplicateAction>,
}

/// ask 策略需要用户选择时返回 NeedsConfirmation；任务尚未写入持久化列表。
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CreateTaskResult {
    Created { task: Box<TaskRecord> },
    NeedsConfirmation { song_title: String },
    Cancelled,
}

#[derive(Debug, Serialize)]
pub struct BatchResult {
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_existing_frontend_task_record() {
        let old_record = serde_json::json!({
            "id": "old-1", "songId": 42, "songMid": "mid",
            "songTitle": "Title", "artist": "Artist", "album": "Album",
            "filename": "song.mp3", "quality": "320kmp3", "status": "error",
            "fileSize": 100, "downloaded": 0, "retryCount": 1, "addedAt": 1234
        });
        let task: TaskRecord = serde_json::from_value(old_record).unwrap();
        assert_eq!(task.platform, "qqmusic");
        assert_eq!(task.song_id, 42);
        assert!(task.cover_url.is_empty());
        assert!(task.media_mid.is_empty());
        assert!(task.available_qualities.is_none());
        assert!(task.save_path.is_none());
        let serialized = serde_json::to_value(&task).unwrap();
        assert_eq!(serialized["platform"], "qqmusic");
        assert_eq!(serialized["songTitle"], "Title");
        assert_eq!(serialized["status"], "error");

        // Box 只改变 Rust 内存布局，不改变 IPC 和 HTTP 共用的响应 JSON。
        let result = serde_json::to_value(CreateTaskResult::Created {
            task: Box::new(task),
        })
        .unwrap();
        assert_eq!(result["outcome"], "created");
        assert_eq!(result["task"], serialized);
    }

    #[test]
    fn loads_mixed_task_history_and_preserves_existing_platforms() {
        let record = serde_json::json!({
            "id": "legacy", "songId": 42, "songMid": "mid",
            "songTitle": "Title", "artist": "Artist", "album": "Album",
            "filename": "song.mp3", "quality": "320kmp3", "status": "completed",
            "filePath": "D:/Music/song.mp3", "fileSize": 100, "downloaded": 100,
            "retryCount": 0, "addedAt": 1234
        });
        let mut qqmusic = record.clone();
        qqmusic["id"] = serde_json::json!("qq");
        qqmusic["platform"] = serde_json::json!("qqmusic");
        let mut kuwo = record.clone();
        kuwo["id"] = serde_json::json!("kw");
        kuwo["platform"] = serde_json::json!("kuwo");
        let json = serde_json::json!([record, qqmusic, kuwo]).to_string();

        let tasks: Vec<TaskRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].platform, "qqmusic");
        assert_eq!(tasks[1].platform, "qqmusic");
        assert_eq!(tasks[2].platform, "kuwo");
        assert_eq!(tasks[0].status, TaskStatus::Completed);
        assert_eq!(tasks[0].file_path.as_deref(), Some("D:/Music/song.mp3"));
        assert_eq!(tasks[0].downloaded, 100);

        let saved = serde_json::to_string(&tasks).unwrap();
        let reloaded: Vec<TaskRecord> = serde_json::from_str(&saved).unwrap();
        assert_eq!(reloaded[0].platform, "qqmusic");
        assert_eq!(reloaded[2].platform, "kuwo");
    }

    #[test]
    fn song_id_normalization_keeps_numbers_and_hashes_other_strings() {
        assert_eq!(song_id_to_u64("42"), 42);
        assert_eq!(song_id_to_u64("  42  "), 42);
        // 咪咕的 contentId 是 18 位数字字符串，必须精确解析（不能截断）。
        assert_eq!(
            song_id_to_u64("600913000007163534"),
            600_913_000_007_163_534
        );
        // 网易云的 id 是数字字符串，与酷我的数字形态等价。
        assert_eq!(song_id_to_u64("2652820720"), 2_652_820_720);
        // 哔哩哔哩的 id 是 BV 号：稳定且不同视频不同值。
        let bvid = song_id_to_u64("BV1BZbSzZEGT");
        assert_eq!(bvid, song_id_to_u64("BV1BZbSzZEGT"));
        assert_ne!(bvid, song_id_to_u64("BV1BZbSzZEGU"));
        assert_eq!(song_id_to_u64(""), song_id_to_u64(""));
    }

    #[test]
    fn create_task_request_accepts_string_song_ids() {
        fn request(song_id: serde_json::Value) -> serde_json::Value {
            serde_json::json!({
                "song": {
                    "platform": "migu",
                    "id": song_id,
                    "mid": "60054701923|600902000006889366",
                    "title": "晴天",
                    "artist": "周杰伦",
                    "album": "叶惠美",
                    "qualities": [{ "quality": "128kmp3", "filename": "PQ.mp3", "size": 4317311 }]
                },
                "desiredQuality": "128kmp3"
            })
        }

        // 数字字符串（网易云/咪咕）精确解析。
        let parsed: CreateTaskRequest =
            serde_json::from_value(request(serde_json::json!("600913000007163534"))).unwrap();
        assert_eq!(parsed.song.id, 600_913_000_007_163_534);

        // 非数字字符串（B 站 BV 号）折算为稳定哈希，不再让整条请求失败。
        let parsed: CreateTaskRequest =
            serde_json::from_value(request(serde_json::json!("BV1BZbSzZEGT"))).unwrap();
        assert_eq!(parsed.song.id, song_id_to_u64("BV1BZbSzZEGT"));

        // 前端已经归一化成数字时照旧。
        let parsed: CreateTaskRequest =
            serde_json::from_value(request(serde_json::json!(42))).unwrap();
        assert_eq!(parsed.song.id, 42);
    }
}
