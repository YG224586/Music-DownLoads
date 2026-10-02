//! 音乐源配置命令：导入 QingMusic 风格 music.json、读取与切换音源开关。
//!
//! 抓取放在这一层（复用 `utils::http::CLIENT`），核心 crate 保持无 HTTP、可单测；
//! 持久化沿用 data.json 的 `sourceConfig` 键，读写都被命令锁串行化。

use std::collections::HashMap;

use tauri::async_runtime::Mutex;
use tauri::AppHandle;

use hotdownloader_core::source_config::{
    build_state, default_source_state, parse_source_config, SourceConfigState,
};

use crate::storage::store_wrapper;
use crate::utils::http::CLIENT;

/// 串行化导入 / 切换，避免两条命令各自读过期状态后互相覆盖。
static SOURCE_COMMAND_LOCK: std::sync::LazyLock<Mutex<()>> =
    std::sync::LazyLock::new(|| Mutex::new(()));

const STORE_KEY: &str = "sourceConfig";

fn load_state(app: &AppHandle) -> Result<SourceConfigState, String> {
    let raw = store_wrapper::load_string(app, STORE_KEY)
        .map_err(|error| format!("读取音源配置失败: {error}"))?;
    if raw.trim().is_empty() {
        return Ok(default_source_state());
    }
    serde_json::from_str(&raw).map_err(|error| format!("音源配置数据损坏: {error}"))
}

fn persist_state(app: &AppHandle, state: &SourceConfigState) -> Result<(), String> {
    let json =
        serde_json::to_string(state).map_err(|error| format!("序列化音源配置失败: {error}"))?;
    store_wrapper::save_string(app, STORE_KEY, &json)
        .map_err(|error| format!("保存音源配置失败: {error}"))
}

/// 读取当前音源配置；未导入过时返回两个内置平台全开的默认状态。
#[tauri::command]
pub async fn get_source_config(app: AppHandle) -> Result<SourceConfigState, String> {
    let _guard = SOURCE_COMMAND_LOCK.lock().await;
    load_state(&app)
}

/// 从 URL 导入 music.json。抓取在此层完成，浏览器端由 mock 服务端转发。
#[tauri::command]
pub async fn import_source_config(
    app: AppHandle,
    url: String,
) -> Result<SourceConfigState, String> {
    let _guard = SOURCE_COMMAND_LOCK.lock().await;

    let trimmed = url.trim().to_string();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("音源配置地址必须以 http:// 或 https:// 开头".to_string());
    }

    let previous = load_state(&app)?;
    // 记录用户历史开关：默认状态与导入配置的音源 id 不同，按 id 合并。
    let overrides: HashMap<String, bool> = previous
        .sources
        .iter()
        .map(|s| (s.id.clone(), s.enabled))
        .collect();

    let response = CLIENT
        .get(&trimmed)
        .send()
        .await
        .map_err(|error| format!("获取音源配置失败: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "获取音源配置失败: HTTP {}",
            response.status().as_u16()
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|error| format!("读取音源配置内容失败: {error}"))?;

    let config = parse_source_config(&body)?;
    let state = build_state(Some(trimmed), &config, &overrides);
    persist_state(&app, &state)?;
    Ok(state)
}

/// 切换某个音源的启用状态；导入前也可对默认两个平台操作。
#[tauri::command]
pub async fn set_source_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<SourceConfigState, String> {
    let _guard = SOURCE_COMMAND_LOCK.lock().await;
    let mut state = load_state(&app)?;
    let source = state
        .sources
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("未找到音源: {id}"))?;
    if source.platform.is_none() && enabled {
        return Err("该音源暂不支持，无法启用".to_string());
    }
    source.enabled = enabled;
    persist_state(&app, &state)?;
    Ok(state)
}
