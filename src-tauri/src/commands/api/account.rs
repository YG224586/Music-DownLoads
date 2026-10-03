//! 账号状态命令。
//!
//! 扫码登录与手动登录入口已按用户要求移除，这里只保留两个最小账号操作：
//! 只读的状态查询，以及清除本地凭据（避免已登录用户再也无法退出）。
//! 歌单页依靠状态查询判断是否展示「我的歌单」。

use tauri::AppHandle;

/// 查询当前登录状态，返回 JSON 字符串 `{"logged_in": bool, "uin": string}`。
///
/// `platform` 参数只为兼容前端既有调用形态保留，当前只有 QQ 音乐使用凭据。
#[tauri::command]
pub async fn get_login_status(app: AppHandle, platform: Option<String>) -> Result<String, String> {
    let _ = platform;
    crate::platforms::qqmusic::login::get_login_status(&app).await
}

/// 清除本地保存的 QQ 凭据；其余设置保持不变。
#[tauri::command]
pub async fn logout(app: AppHandle, platform: Option<String>) -> Result<(), String> {
    let _ = platform;
    crate::platforms::qqmusic::login::logout(&app).await
}
