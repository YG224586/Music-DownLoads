//! 歌词获取命令路由层

use hotdownloader_core::platforms::lyric::LyricData;
use hotdownloader_core::platforms::{self, Platform};
use tauri::command;

/// Tauri 命令：获取歌词。
#[command]
pub async fn get_lyric_by_id(platform: String, song_id: u64) -> Result<LyricData, String> {
    match platform.parse::<Platform>()? {
        Platform::QqMusic => platforms::qqmusic::lyrics::get_lyric_by_id(song_id).await,
        Platform::Kuwo => platforms::kuwo::lyrics::get_lyric_by_id(song_id).await,
    }
}
