//! 逐条读取历史任务，避免一条异常记录阻断全部任务的恢复。

use std::collections::HashSet;

use super::contract::TaskRecord;
use crate::platforms::Platform;

#[derive(Default)]
pub struct TaskRecovery {
    pub tasks: Vec<TaskRecord>,
    pub migrated: usize,
    pub errors: Vec<String>,
}

pub fn recover_tasks(json: &str) -> TaskRecovery {
    if json.is_empty() {
        return TaskRecovery::default();
    }
    let records = match serde_json::from_str::<Vec<serde_json::Value>>(json) {
        Ok(records) => records,
        Err(error) => {
            return TaskRecovery {
                errors: vec![format!("任务列表无法解析: {error}")],
                ..Default::default()
            };
        }
    };
    let mut recovery = TaskRecovery::default();
    let mut ids = HashSet::new();
    for (index, record) in records.into_iter().enumerate() {
        let missing_platform = record.is_object() && record.get("platform").is_none();
        let parsed = serde_json::from_value::<TaskRecord>(record).and_then(|task| {
            let error = if task.id.trim().is_empty() {
                Some("任务 ID 为空".to_owned())
            } else if let Err(error) = task.platform.parse::<Platform>() {
                Some(error)
            } else if !ids.insert(task.id.clone()) {
                Some("任务 ID 重复".to_owned())
            } else {
                None
            };
            match error {
                Some(error) => Err(<serde_json::Error as serde::de::Error>::custom(error)),
                None => Ok(task),
            }
        });
        match parsed {
            Ok(task) => {
                recovery.migrated += usize::from(missing_platform);
                recovery.tasks.push(task);
            }
            Err(error) => recovery
                .errors
                .push(format!("第 {} 条任务记录: {error}", index + 1)),
        }
    }
    recovery
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn record(id: &str) -> Value {
        json!({
            "id": id, "platform": "qqmusic", "songId": 42, "songMid": "mid",
            "songTitle": "Title", "artist": "Artist", "album": "Album",
            "filename": "song.mp3", "quality": "320kmp3", "status": "completed",
            "filePath": "D:/Music/song.mp3", "fileSize": 100, "downloaded": 100,
            "retryCount": 0, "addedAt": 1234
        })
    }

    #[test]
    fn reads_empty_and_current_history_without_recovery() {
        for raw in ["", "[]"] {
            let result = recover_tasks(raw);
            assert!(result.tasks.is_empty());
            assert!(result.errors.is_empty());
            assert_eq!(result.migrated, 0);
        }
        let mut kuwo = record("kw");
        kuwo["platform"] = json!("kuwo");
        let result = recover_tasks(&json!([record("qq"), kuwo]).to_string());
        assert_eq!(result.tasks.len(), 2);
        assert_eq!(result.tasks[1].platform, "kuwo");
        assert!(result.errors.is_empty());
        assert_eq!(result.migrated, 0);
    }

    #[test]
    fn migrates_missing_platform_and_keeps_completed_file_information() {
        let mut legacy = record("legacy");
        legacy.as_object_mut().unwrap().remove("platform");
        let result = recover_tasks(&json!([legacy]).to_string());
        assert_eq!(result.migrated, 1);
        assert!(result.errors.is_empty());
        assert_eq!(result.tasks[0].platform, "qqmusic");
        assert_eq!(
            result.tasks[0].file_path.as_deref(),
            Some("D:/Music/song.mp3")
        );
        assert_eq!(result.tasks[0].downloaded, 100);
    }

    #[test]
    fn isolates_bad_rows_and_preserves_valid_order() {
        let mut missing = record("missing");
        missing.as_object_mut().unwrap().remove("filename");
        let mut wrong_type = record("type");
        wrong_type["fileSize"] = json!("broken");
        let mut status = record("status");
        status["status"] = json!("unknown");
        let result = recover_tasks(
            &json!([
                record("first"),
                missing,
                null,
                wrong_type,
                status,
                record("last")
            ])
            .to_string(),
        );
        assert_eq!(
            result
                .tasks
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "last"]
        );
        assert_eq!(result.errors.len(), 4);
        assert!(result.errors[0].contains("第 2 条"));
    }

    #[test]
    fn malformed_json_and_non_array_roots_are_reported() {
        for raw in ["[{", "null", "{}", "42", "\"text\"", " "] {
            let result = recover_tasks(raw);
            assert!(result.tasks.is_empty(), "{raw}");
            assert_eq!(result.errors.len(), 1, "{raw}");
        }
    }

    #[test]
    fn isolates_duplicate_or_empty_ids_and_invalid_platforms() {
        let mut null_platform = record("null");
        null_platform["platform"] = Value::Null;
        let mut unknown = record("unknown");
        unknown["platform"] = json!("other");
        let mut empty_platform = record("empty");
        empty_platform["platform"] = json!("");
        let result = recover_tasks(
            &json!([
                record("ok"),
                record("ok"),
                record(" "),
                null_platform,
                unknown,
                empty_platform
            ])
            .to_string(),
        );
        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.errors.len(), 5);
    }

    #[test]
    fn preserves_other_rows_when_early_mid_format_cannot_be_migrated() {
        let mut legacy = record("v1.0.3");
        legacy["songId"] = json!("003legacyMid");
        legacy.as_object_mut().unwrap().remove("songMid");
        legacy.as_object_mut().unwrap().remove("platform");
        let result = recover_tasks(&json!([legacy, record("current")]).to_string());
        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.tasks[0].id, "current");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.migrated, 0);
    }
}
