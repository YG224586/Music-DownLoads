//! 在 Store 加载前检查完整文件。恢复写入必须先备份原始字节，失败时保持只读。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use hotdownloader_core::task::recovery::recover_tasks;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReport {
    pub recovered_count: usize,
    pub migrated_count: usize,
    pub errors: Vec<String>,
    pub backup_path: Option<String>,
    pub read_only: bool,
}

pub struct RecoveredStore {
    pub values: Map<String, Value>,
    pub report: Option<RecoveryReport>,
}

/// 读、备份和提交通过闭包提供，保证失败顺序也能在测试中验证。
pub fn recover_store(
    raw: Result<Option<Vec<u8>>, String>,
    backup: impl FnOnce(&[u8]) -> Result<String, String>,
    persist: impl FnOnce(&Map<String, Value>) -> Result<(), String>,
) -> RecoveredStore {
    let bytes = match raw {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return RecoveredStore {
                values: Map::new(),
                report: None,
            }
        }
        Err(error) => {
            return RecoveredStore {
                values: Map::new(),
                report: Some(RecoveryReport {
                    recovered_count: 0,
                    migrated_count: 0,
                    errors: vec![error],
                    backup_path: None,
                    read_only: true,
                }),
            };
        }
    };
    let (mut values, mut errors) = match serde_json::from_slice::<Map<String, Value>>(&bytes) {
        Ok(values) => (values, Vec::new()),
        Err(error) => (Map::new(), vec![format!("data.json 无法解析: {error}")]),
    };
    let mut recovered_count = 0;
    let mut migrated_count = 0;
    if let Some(raw_tasks) = values.get("tasks") {
        match raw_tasks.as_str() {
            Some(json) => {
                let recovery = recover_tasks(json);
                recovered_count = recovery.tasks.len();
                migrated_count = recovery.migrated;
                errors.extend(recovery.errors);
                if migrated_count > 0 || !errors.is_empty() {
                    // TaskRecord 序列化仅包含普通字符串和整数，不会产生无效 JSON。
                    values.insert(
                        "tasks".into(),
                        Value::String(
                            serde_json::to_string(&recovery.tasks).expect("serialize task records"),
                        ),
                    );
                }
            }
            None => {
                errors.push("tasks 应为 JSON 字符串，已隔离异常任务数据".into());
                values.insert("tasks".into(), Value::String("[]".into()));
            }
        }
    }
    if errors.is_empty() && migrated_count == 0 {
        return RecoveredStore {
            values,
            report: None,
        };
    }
    let mut report = RecoveryReport {
        recovered_count,
        migrated_count,
        errors,
        backup_path: None,
        read_only: false,
    };
    match backup(&bytes) {
        Ok(path) => {
            report.backup_path = Some(path);
            if let Err(error) = persist(&values) {
                report.errors.push(format!("保存恢复结果失败: {error}"));
                report.read_only = true;
            }
        }
        Err(error) => {
            report.errors.push(format!("备份原始数据失败: {error}"));
            report.read_only = true;
        }
    }
    RecoveredStore {
        values,
        report: Some(report),
    }
}

pub fn read_store(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("读取 data.json 失败: {error}")),
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("创建 {} 失败: {error}", path.display()))?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("写入 {} 失败: {error}", path.display()));
    }
    Ok(())
}

pub fn backup_store(path: &Path, bytes: &[u8]) -> Result<String, String> {
    let directory = path.parent().ok_or("存储文件缺少父目录")?.join("recovery");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let backup = directory.join(format!("data-{:016x}.json.bak", rand::random::<u64>()));
    write_synced(&backup, bytes)?;
    Ok(backup.to_string_lossy().into_owned())
}

/// 同目录临时文件写完并同步后替换目标，避免 Store 的截断写入损坏原文件。
pub fn persist_store(path: &Path, values: &Map<String, Value>) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(values).map_err(|error| error.to_string())?;
    let directory = path.parent().ok_or("存储文件缺少父目录")?;
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let temp: PathBuf = directory.join(format!(".data-{:016x}.tmp", rand::random::<u64>()));
    write_synced(&temp, &bytes)?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(format!("替换 data.json 失败: {error}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    fn task(id: &str) -> Value {
        json!({
            "id": id, "platform": "qqmusic", "songId": 42, "songMid": "mid",
            "songTitle": "Title", "artist": "Artist", "album": "Album",
            "filename": "song.mp3", "quality": "320kmp3", "status": "completed",
            "fileSize": 100, "downloaded": 100, "retryCount": 0, "addedAt": 1234
        })
    }

    fn stored(tasks: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "tasks": tasks.to_string(), "settings": "{\"maxConcurrent\":5}",
            "history": "[\"keyword\"]", "extra": {"keep": true}
        }))
        .unwrap()
    }

    #[test]
    fn new_and_valid_stores_do_not_write_or_backup() {
        for bytes in [
            None,
            Some(stored(json!([task("ok")]))),
            Some(b"{}".to_vec()),
        ] {
            let result = recover_store(
                Ok(bytes),
                |_| panic!("unexpected backup"),
                |_| panic!("unexpected write"),
            );
            assert!(result.report.is_none());
        }
    }

    #[test]
    fn backs_up_before_saving_partial_recovery_and_keeps_other_keys() {
        let raw = stored(json!([task("ok"), {"id":"bad"}]));
        let calls = RefCell::new(Vec::new());
        let result = recover_store(
            Ok(Some(raw.clone())),
            |bytes| {
                assert_eq!(bytes, raw);
                calls.borrow_mut().push("backup");
                Ok("backup.json.bak".into())
            },
            |values| {
                calls.borrow_mut().push("save");
                assert_eq!(values["settings"], "{\"maxConcurrent\":5}");
                assert_eq!(values["history"], "[\"keyword\"]");
                assert_eq!(values["extra"], json!({"keep":true}));
                let tasks: Vec<Value> =
                    serde_json::from_str(values["tasks"].as_str().unwrap()).unwrap();
                assert_eq!(tasks.len(), 1);
                assert_eq!(tasks[0]["id"], "ok");
                Ok(())
            },
        );
        assert_eq!(*calls.borrow(), ["backup", "save"]);
        let report = result.report.unwrap();
        assert_eq!(report.recovered_count, 1);
        assert_eq!(report.errors.len(), 1);
        assert!(!report.read_only);
    }

    #[test]
    fn migration_is_backed_up_and_saved_once() {
        let mut legacy = task("legacy");
        legacy.as_object_mut().unwrap().remove("platform");
        let saved = RefCell::new(Map::new());
        let result = recover_store(
            Ok(Some(stored(json!([legacy])))),
            |_| Ok("backup".into()),
            |values| {
                *saved.borrow_mut() = values.clone();
                Ok(())
            },
        );
        assert_eq!(result.report.unwrap().migrated_count, 1);
        let raw = serde_json::to_vec(&*saved.borrow()).unwrap();
        let reloaded = recover_store(
            Ok(Some(raw)),
            |_| panic!("second backup"),
            |_| panic!("second write"),
        );
        assert!(reloaded.report.is_none());
    }

    #[test]
    fn corrupt_outer_file_is_backed_up_byte_for_byte() {
        for bytes in [
            b"{broken".to_vec(),
            vec![0xff, 0xfe],
            b"null".to_vec(),
            b"[]".to_vec(),
            Vec::new(),
        ] {
            let result = recover_store(
                Ok(Some(bytes.clone())),
                |original| {
                    assert_eq!(original, bytes);
                    Ok("backup".into())
                },
                |values| {
                    assert!(values.is_empty());
                    Ok(())
                },
            );
            let report = result.report.unwrap();
            assert_eq!(report.recovered_count, 0);
            assert!(!report.errors.is_empty());
            assert!(!report.read_only);
        }
    }

    #[test]
    fn corrupt_task_json_and_wrong_store_field_types_are_isolated() {
        for tasks in [json!("[{"), Value::Null, json!([]), json!(42)] {
            let raw = serde_json::to_vec(&json!({"tasks":tasks,"settings":"{}"})).unwrap();
            let result = recover_store(Ok(Some(raw)), |_| Ok("backup".into()), |_| Ok(()));
            assert_eq!(result.values["tasks"], "[]");
            assert_eq!(result.values["settings"], "{}");
            assert_eq!(result.report.unwrap().errors.len(), 1);
        }
    }

    #[test]
    fn failed_backup_blocks_commit_but_keeps_valid_tasks_in_memory() {
        let result = recover_store(
            Ok(Some(stored(json!([task("ok"), null])))),
            |_| Err("disk full".into()),
            |_| panic!("must not overwrite original"),
        );
        assert!(result.values["tasks"].as_str().unwrap().contains("ok"));
        let report = result.report.unwrap();
        assert!(report.read_only);
        assert!(report.backup_path.is_none());
        assert!(report.errors.last().unwrap().contains("disk full"));
    }

    #[test]
    fn failed_commit_keeps_backup_and_enters_read_only_mode() {
        let result = recover_store(
            Ok(Some(stored(json!([null])))),
            |_| Ok("backup".into()),
            |_| Err("permission denied".into()),
        );
        let report = result.report.unwrap();
        assert!(report.read_only);
        assert_eq!(report.backup_path.as_deref(), Some("backup"));
        assert!(report.errors.last().unwrap().contains("permission denied"));
    }

    #[test]
    fn read_failure_does_not_attempt_backup_or_overwrite() {
        let result = recover_store(
            Err("permission denied".into()),
            |_| panic!("no readable original"),
            |_| panic!("must not write"),
        );
        assert!(result.report.unwrap().read_only);
    }

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "hotdownloader-recovery-{:x}",
                rand::random::<u64>()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn real_files_preserve_original_and_reload_repaired_history() {
        let directory = TestDirectory::new();
        let path = directory.0.join("data.json");
        let raw = stored(json!([task("ok"), null]));
        fs::write(&path, &raw).unwrap();
        let result = recover_store(
            read_store(&path),
            |bytes| backup_store(&path, bytes),
            |values| persist_store(&path, values),
        );
        let backup = result.report.unwrap().backup_path.unwrap();
        assert_eq!(fs::read(backup).unwrap(), raw);
        let second = recover_store(
            read_store(&path),
            |_| panic!("already repaired"),
            |_| panic!("already repaired"),
        );
        assert!(second.report.is_none());
        assert_eq!(second.values, result.values);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }

    #[test]
    fn real_backup_failure_leaves_original_file_unchanged() {
        let directory = TestDirectory::new();
        let path = directory.0.join("data.json");
        let raw = stored(json!([task("ok"), null]));
        fs::write(&path, &raw).unwrap();
        // 同名文件阻止创建备份目录，模拟备份目标不可用。
        fs::write(directory.0.join("recovery"), b"blocker").unwrap();
        let result = recover_store(
            read_store(&path),
            |bytes| backup_store(&path, bytes),
            |_| panic!("backup failed"),
        );
        assert!(result.report.unwrap().read_only);
        assert_eq!(fs::read(path).unwrap(), raw);
    }

    #[test]
    fn failed_atomic_replace_keeps_destination_and_removes_temporary_file() {
        let directory = TestDirectory::new();
        let target = directory.0.join("data.json");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"original").unwrap();
        assert!(persist_store(&target, &Map::new()).is_err());
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"original");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn backup_collision_does_not_overwrite_existing_file() {
        let directory = TestDirectory::new();
        let path = directory.0.join("backup");
        fs::write(&path, b"original").unwrap();
        assert!(write_synced(&path, b"replacement").is_err());
        assert_eq!(fs::read(path).unwrap(), b"original");
    }
}
