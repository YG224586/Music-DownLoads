//! 歌单导入命令路由层

use crate::utils::settings::get_artist_separator;
use hotdownloader_core::platforms::{self, Platform};
use tauri::{command, AppHandle};

#[command]
pub async fn fetch_playlist_songs(
    app: AppHandle,
    platform: String,
    input: String,
) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    // 歌单解析需要用户设置的歌手分隔符，具体请求无需接触 AppHandle。
    let separator = get_artist_separator(&app);
    match p {
        Platform::QqMusic => {
            platforms::qqmusic::playlist::fetch_playlist_songs(&separator, input).await
        }
        Platform::Kuwo => platforms::kuwo::playlist::fetch_playlist_songs(&separator, input).await,
    }
}

/// 搜索歌单命令路由层。
///
/// 接收平台标识、搜索关键词、页码（从 1 开始）和每页数量，调用对应平台的歌单搜索实现。
/// 返回 JSON 字符串，格式为 `{"playlists": [...], "has_more": true/false}`。
#[command]
pub async fn search_playlists(
    platform: String,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let p = Platform::from_str(&platform)?;
    match p {
        Platform::QqMusic => {
            platforms::qqmusic::playlist::search_playlists(keyword, page, limit).await
        }
        Platform::Kuwo => platforms::kuwo::playlist::search_playlists(keyword, page, limit).await,
    }
}

/// 读取当前 QQ 登录用户创建的歌单；UIN 从 Tauri 登录存储取得。
#[command]
pub async fn fetch_created_playlists(app: AppHandle) -> Result<String, String> {
    crate::platforms::qqmusic::login::fetch_created_playlists(&app).await
}

/// 读取个人歌单详情，使用 ID 与目录 ID 定位歌单。
#[command]
pub async fn fetch_created_playlist_songs(
    app: AppHandle,
    id: String,
    dirid: String,
) -> Result<String, String> {
    let separator = get_artist_separator(&app);
    crate::platforms::qqmusic::login::fetch_created_playlist_songs(&app, &separator, id, dirid)
        .await
}
