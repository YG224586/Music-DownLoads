use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::{Store, StoreExt};

use super::recovery::{backup_store, persist_store, read_store, recover_store, RecoveryReport};

struct WriteState {
    read_only: bool,
    report: Option<RecoveryReport>,
}

impl WriteState {
    fn ensure_writable(&self) -> Result<(), String> {
        if self.read_only {
            Err("本地数据处于只读恢复模式，暂时无法保存。请检查备份或磁盘权限后重启应用。".into())
        } else {
            Ok(())
        }
    }

    fn failed_write(&mut self, error: &str, task_count: usize) {
        self.read_only = true;
        let report = self.report.get_or_insert_with(|| RecoveryReport {
            recovered_count: task_count,
            migrated_count: 0,
            errors: Vec::new(),
            backup_path: None,
            read_only: true,
        });
        report.read_only = true;
        report.errors.push(format!("保存本地数据失败: {error}"));
    }
}

struct AppStore {
    store: Arc<Store<tauri::Wry>>,
    path: PathBuf,
    // 所有任务、设置、历史写入共用一把锁，检查只读策略后才允许写盘。
    writes: Mutex<WriteState>,
}

/// 必须在启动加载任务、设置之前调用，避免插件忽略加载错误后覆盖损坏文件。
pub fn initialize(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let path = app.path().app_data_dir()?.join("data.json");
    let recovered = recover_store(
        read_store(&path),
        |bytes| backup_store(&path, bytes),
        |values| persist_store(&path, values),
    );
    let read_only = recovered
        .report
        .as_ref()
        .is_some_and(|report| report.read_only);
    if let Some(report) = &recovered.report {
        log::warn!(
            "本地数据恢复: 恢复 {} 条任务，迁移 {} 条任务，{} 项异常，只读: {}，备份: {:?}",
            report.recovered_count,
            report.migrated_count,
            report.errors.len(),
            report.read_only,
            report.backup_path
        );
    }
    // 使用检查后的内存快照；所有保存都经原子写入，关闭插件自动保存。
    let store = app
        .store_builder("data.json")
        .create_new()
        .defaults(recovered.values.into_iter().collect())
        .disable_auto_save()
        .build()?;
    app.manage(AppStore {
        store,
        path,
        writes: Mutex::new(WriteState {
            read_only,
            report: recovered.report,
        }),
    });
    Ok(())
}

pub fn recovery_report(app: &AppHandle) -> Option<RecoveryReport> {
    app.state::<AppStore>()
        .writes
        .lock()
        .unwrap()
        .report
        .clone()
}

/// 从默认 data.json 存储加载字符串。
pub fn load_string(app: &AppHandle, key: &str) -> Result<String, Box<dyn std::error::Error>> {
    let state = app.state::<AppStore>();
    Ok(state
        .store
        .get(key)
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default())
}

fn save_value(
    state: &AppStore,
    writes: &mut WriteState,
    key: &str,
    value: String,
) -> Result<(), String> {
    writes.ensure_writable()?;
    let mut next: Map<String, Value> = state.store.entries().into_iter().collect();
    next.insert(key.into(), Value::String(value.clone()));
    if let Err(error) = persist_store(&state.path, &next) {
        let task_count = state
            .store
            .get("tasks")
            .and_then(|value| serde_json::from_str::<Vec<Value>>(value.as_str()?).ok())
            .map_or(0, |tasks| tasks.len());
        writes.failed_write(&error, task_count);
        return Err(error);
    }
    // 写盘成功后才发布缓存，失败时仍保留原缓存和原文件。
    state.store.set(key.to_owned(), Value::String(value));
    Ok(())
}

/// 保存字符串到默认 data.json 存储。
pub fn save_string(
    app: &AppHandle,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = app.state::<AppStore>();
    let mut writes = state
        .writes
        .lock()
        .map_err(|_| std::io::Error::other("存储写入锁已损坏"))?;
    save_value(&state, &mut writes, key, value.to_owned())
        .map_err(|error| std::io::Error::other(error).into())
}

/// 设置与登录凭据刷新共用此入口，避免各自读取旧 JSON 后覆盖另一方的字段。
pub fn update_settings(
    app: &AppHandle,
    update: impl FnOnce(&str) -> Result<String, String>,
) -> Result<String, String> {
    let state = app.state::<AppStore>();
    let mut writes = state
        .writes
        .lock()
        .map_err(|_| "存储写入锁已损坏".to_string())?;
    writes.ensure_writable()?;
    let previous = state
        .store
        .get("settings")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let next = update(&previous)?;
    if next != previous {
        save_value(&state, &mut writes, "settings", next.clone())?;
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_read_only_policy_blocks_all_writes() {
        let state = WriteState {
            read_only: true,
            report: None,
        };
        assert!(state.ensure_writable().unwrap_err().contains("只读恢复"));
    }

    #[test]
    fn failed_write_blocks_later_writes_and_exposes_reason() {
        let mut state = WriteState {
            read_only: false,
            report: None,
        };
        assert!(state.ensure_writable().is_ok());
        state.failed_write("disk full", 3);
        assert!(state.ensure_writable().is_err());
        let report = state.report.unwrap();
        assert!(report.read_only);
        assert_eq!(report.recovered_count, 3);
        assert!(report.errors[0].contains("disk full"));
    }
}
