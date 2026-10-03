//! 热搜与搜索建议命令路由层
//!
//! 热词与联想只对 QQ 音乐、酷我音乐开放；新增平台暂未实现，统一返回明确错误。

use hotdownloader_core::platforms::{self, Platform};
use tauri::command;

use crate::utils::platform_caps::{unsupported, SCRIPT_UNSUPPORTED};

#[command]
pub async fn fetch_hot_keywords(platform: String) -> Result<String, String> {
    let p = platform.parse::<Platform>()?;
    match p {
        Platform::QqMusic => platforms::qqmusic::suggest::fetch_hot_keywords().await,
        Platform::Kuwo => platforms::kuwo::suggest::fetch_hot_keywords().await,
        Platform::Script(_) => Err(SCRIPT_UNSUPPORTED.into()),
        other => Err(unsupported(other, "热门搜索")),
    }
}

#[command]
pub async fn fetch_suggestions(platform: String, keyword: String) -> Result<String, String> {
    let p = platform.parse::<Platform>()?;
    match p {
        Platform::QqMusic => platforms::qqmusic::suggest::fetch_suggestions(keyword).await,
        Platform::Kuwo => platforms::kuwo::suggest::fetch_suggestions(keyword).await,
        Platform::Script(_) => Err(SCRIPT_UNSUPPORTED.into()),
        other => Err(unsupported(other, "搜索建议")),
    }
}
