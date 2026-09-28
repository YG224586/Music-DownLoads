//! Tauri 只负责读取持久化设置；分隔符默认值与校验位于共享核心。

use hotdownloader_core::settings::artist::{separator, DEFAULT_SEPARATOR};
use tauri::AppHandle;

use crate::storage::store_wrapper;

/// 从设置中读取歌手分隔符。
///
/// 解析失败、缺失或为空时，回退到 [`DEFAULT_SEPARATOR`]。
/// 存储读取失败记录 warn；JSON 解析失败静默回退，避免在搜索和歌单等
/// 高频调用路径上反复产生相同日志。
pub fn get_artist_separator(app_handle: &AppHandle) -> String {
    let raw = match store_wrapper::load_string(app_handle, "settings") {
        Ok(s) => s,
        Err(e) => {
            log::warn!("读取歌手分隔符设置失败: {}, 使用默认值", e);
            return DEFAULT_SEPARATOR.to_string();
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return DEFAULT_SEPARATOR.to_string(),
    };

    separator(&parsed).to_string()
}
