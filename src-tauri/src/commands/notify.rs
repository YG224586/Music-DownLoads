//! 系统通知模块。
//!
//! 封装下载完成等事件的通知发送逻辑，使用 tauri-plugin-notification
//! 实现跨平台统一的系统通知，提升用户体验。

use tauri::AppHandle;
use tauri_plugin_notification::{NotificationExt, PermissionState};

/// 发送下载完成系统通知。
///
/// 该函数在后台任务成功完成（音频文件、metadata 和 LRC 全部处理完毕）时调用，
/// 通过系统通知告知用户下载结果。通知标题包含歌曲标题和艺术家。
///
/// # 参数
/// - `app`: Tauri 应用句柄，用于获取通知管理器。
/// - `song_title`: 歌曲标题。
/// - `artist`: 歌手名称。
///
/// # 注意
/// - 通知发送失败不会影响主流程，仅记录错误日志。
/// - Android 端点击通知的行为由系统处理。
pub fn send_download_complete_notification(app: &AppHandle, song_title: &str, artist: &str) {
    // 构建通知标题和内容
    let title = format!("下载完成：{}", song_title);
    let body = format!("{} - {}", song_title, artist);

    // 使用通知插件发送系统通知
    if let Err(e) = app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
    {
        log::error!("发送下载完成通知失败: {}", e);
    } else {
        log::info!("已发送下载完成通知: {}", title);
    }
}

/// 请求系统通知权限（主要在 Android 13+ 需要）。
///
/// 该函数通过 tauri-plugin-notification 提供的 API 请求通知权限。
/// 在 Android 上，若用户未授予 POST_NOTIFICATIONS 权限，通知将无法显示。
/// 在其他平台，该方法通常直接返回已授权状态。
///
/// # 参数
/// - `app`: Tauri 应用句柄。
///
/// # 返回
/// - `Ok(bool)`: `true` 表示权限已授予，`false` 表示权限被拒绝。
/// - `Err(String)`: 请求过程中发生错误（如插件不支持）。
#[tauri::command]
pub fn request_notification_permission(app: AppHandle) -> Result<bool, String> {
    // ===== 请求通知权限 =====
    // 在用户首次启用通知功能时触发系统权限弹窗，避免后续发送通知因权限不足而静默失败。
    app.notification()
        .request_permission()
        .map(|state| state == PermissionState::Granted)
        .map_err(|e| format!("请求通知权限失败: {}", e))
}

/// 检查系统通知权限状态（主要用于 Android 13+）。
///
/// 该函数通过 tauri-plugin-notification 查询当前通知权限是否已被授予。
/// 在 Android 上，权限状态决定通知能否显示；其他平台通常返回已授权。
///
/// # 参数
/// - `app`: Tauri 应用句柄。
///
/// # 返回
/// - `Ok(bool)`: `true` 表示权限已授予，`false` 表示未授予或无法确定。
/// - `Err(String)`: 查询过程中发生错误。
#[tauri::command]
pub fn check_notification_permission(app: AppHandle) -> Result<bool, String> {
    // ===== 检查通知权限状态 =====
    // 在用户再次开启通知开关时，先检查权限是否已经授予，避免重复调用 request_permission 导致 Android 系统卡死。
    app.notification()
        .permission_state()
        .map(|state| state == PermissionState::Granted)
        .map_err(|e| format!("检查通知权限失败: {}", e))
}
