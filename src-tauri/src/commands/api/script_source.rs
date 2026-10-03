//! 自定义音源脚本命令：列表 / 安装 / 开关 / 删除 / 测试 / 搜索。
//!
//! 契约：`_dev/script-spec/API.md` §7。脚本正文与启用状态由 Rust 持久化
//! （`store_wrapper` 键 `scriptSources`），前端只拿元信息（不含正文）。

use tauri::async_runtime::Mutex;
use tauri::AppHandle;

use hotdownloader_core::platforms::{Platform, ScriptId};
use hotdownloader_core::script as script_sources;
use hotdownloader_core::script::{ScriptSong, ScriptSourceState, ScriptTestReport};

use crate::storage::store_wrapper;

/// 串行化脚本音源的读改写，避免两条命令各自读过期状态后互相覆盖。
static SCRIPT_COMMAND_LOCK: std::sync::LazyLock<Mutex<()>> =
    std::sync::LazyLock::new(|| Mutex::new(()));

const STORE_KEY: &str = "scriptSources";

/// 启动时把持久化的音源灌回进程内注册表（必须在 `store_wrapper::initialize` 之后调用）。
pub fn restore_sources(app: &AppHandle) {
    let raw = match store_wrapper::load_string(app, STORE_KEY) {
        Ok(raw) => raw,
        Err(error) => {
            log::warn!("读取自定义音源失败: {error}");
            return;
        }
    };
    let restored = script_sources::restore_from_json(&raw);
    if restored > 0 {
        log::info!("已恢复 {restored} 个自定义音源");
    }
}

fn persist(app: &AppHandle) -> Result<(), String> {
    let json = script_sources::sources_json();
    store_wrapper::save_string(app, STORE_KEY, &json)
        .map_err(|error| format!("保存音源脚本失败: {error}"))
}

/// 列出已安装的自定义音源（不含脚本正文）。
#[tauri::command]
pub async fn list_script_sources() -> Result<ScriptSourceState, String> {
    Ok(script_sources::list_sources())
}

/// 安装（或按同名覆盖）一个自定义音源。
#[tauri::command]
pub async fn install_script_source(
    app: AppHandle,
    name: Option<String>,
    script: String,
    url: Option<String>,
) -> Result<ScriptSourceState, String> {
    let _guard = SCRIPT_COMMAND_LOCK.lock().await;
    script_sources::install_script(script, name, url).await?;
    persist(&app)?;
    Ok(script_sources::list_sources())
}

/// 启用 / 停用某个自定义音源。
#[tauri::command]
pub async fn set_script_source_enabled(
    app: AppHandle,
    id: u32,
    enabled: bool,
) -> Result<ScriptSourceState, String> {
    let _guard = SCRIPT_COMMAND_LOCK.lock().await;
    let state = script_sources::set_enabled(id, enabled)?;
    persist(&app)?;
    Ok(state)
}

/// 删除某个自定义音源。
#[tauri::command]
pub async fn remove_script_source(app: AppHandle, id: u32) -> Result<ScriptSourceState, String> {
    let _guard = SCRIPT_COMMAND_LOCK.lock().await;
    let state = script_sources::remove_source(id)?;
    persist(&app)?;
    Ok(state)
}

/// 测试脚本：默认只做静态校验；给了 `keyword` 就额外跑一次搜索并把首条结果放进 `sample`。
#[tauri::command]
pub async fn test_script_source(
    script: String,
    keyword: Option<String>,
) -> Result<ScriptTestReport, String> {
    Ok(script_sources::test_script(script, keyword).await)
}

/// 用某个已安装音源搜索（返回与内置平台同构的 `SearchResponse` JSON）。
#[tauri::command]
pub async fn search_script_source(
    id: u32,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let limit = script_sources::normalize_limit(limit);
    let page = script_sources::normalize_page(page);
    let songs = script_sources::search_source(id, keyword, page, limit).await?;
    Ok(search_response_json(id, &songs, limit))
}

/// 脚本歌曲 → 前端 `SongInfo`（`platform` 用 `"script:<id>"`，`mid` 保留脚本原始 id）。
fn search_response_json(source_id: u32, songs: &[ScriptSong], limit: u32) -> String {
    let platform = Platform::Script(ScriptId::new(source_id)).to_string();
    let items: Vec<serde_json::Value> = songs
        .iter()
        .map(|song| {
            let qualities: Vec<serde_json::Value> = script_sources::quality_items(&song.qualities)
                .into_iter()
                .map(|item| {
                    serde_json::json!({
                        "quality": item.quality,
                        "filename": item.filename,
                        "size": item.size,
                    })
                })
                .collect();
            serde_json::json!({
                "platform": platform,
                "id": script_sources::song_numeric_id(&song.id),
                "mid": song.id,
                "title": song.title,
                "artist": song.artist,
                "album": song.album.clone().unwrap_or_default(),
                "coverUrl": song.cover.clone().unwrap_or_default(),
                "mediaMid": "",
                "qualities": qualities,
            })
        })
        .collect();
    let payload = serde_json::json!({
        "songs": items,
        "has_more": songs.len() >= limit as usize,
    });
    payload.to_string()
}
