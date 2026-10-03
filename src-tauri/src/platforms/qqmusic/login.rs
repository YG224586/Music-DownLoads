//! QQ 凭据的 Tauri 存储适配器。读取、校验与刷新策略位于共享核心。

use hotdownloader_core::platforms::qqmusic::login::{
    self as qq_login, LoginCredentialStore, ResolvedDownloadAuth,
};
use serde_json::Value;
use tauri::AppHandle;

use crate::storage::store_wrapper;

struct TauriLoginStore {
    app: AppHandle,
}

impl TauriLoginStore {
    fn new(app: &AppHandle) -> Self {
        Self { app: app.clone() }
    }
}

impl LoginCredentialStore for TauriLoginStore {
    fn load(&self) -> Result<Value, String> {
        // 旧客户端把整份 settings JSON 存在 Tauri Store 的 settings 键中。
        // 缺少键时按空设置处理；JSON 损坏应明确报错，避免覆盖其他设置。
        let raw = store_wrapper::load_string(&self.app, "settings")
            .map_err(|error| format!("读取登录设置失败: {error}"))?;
        if raw.is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_str(&raw).map_err(|error| format!("解析登录设置失败: {error}"))
    }

    fn save(&self, settings: &Value) -> Result<(), String> {
        // 登录核心给出的是它读取时的整份设置。只合并凭据字段，
        // 使等待网络刷新的这段时间里用户修改的下载选项仍然有效。
        store_wrapper::update_settings(&self.app, |previous| {
            let mut latest: Value = if previous.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(previous)
                    .map_err(|error| format!("解析登录设置失败: {error}"))?
            };
            let target = latest.as_object_mut().ok_or("设置必须是 JSON 对象")?;
            for key in [
                "loginUin",
                "authst",
                "refreshToken",
                "refreshKey",
                "accessToken",
                "openid",
                "loginResponseData",
            ] {
                match settings.get(key) {
                    Some(value) => {
                        target.insert(key.to_string(), value.clone());
                    }
                    None => {
                        target.remove(key);
                    }
                }
            }
            Ok(latest.to_string())
        })?;
        Ok(())
    }
}

/// 使用应用 Store 提供凭据；校验与刷新策略由核心处理。
pub(crate) async fn download_auth(app: &AppHandle) -> ResolvedDownloadAuth {
    qq_login::download_auth(&TauriLoginStore::new(app)).await
}

/// 只读的登录状态查询；不会写入或清除任何凭据。
pub(crate) async fn get_login_status(app: &AppHandle) -> Result<String, String> {
    qq_login::get_login_status(&TauriLoginStore::new(app)).await
}

/// 清除本地保存的登录凭据（保留其余设置）。登录入口已移除，这是唯一的账号写操作。
pub(crate) async fn logout(app: &AppHandle) -> Result<(), String> {
    qq_login::logout(&TauriLoginStore::new(app)).await
}

pub(crate) async fn fetch_created_playlists(app: &AppHandle) -> Result<String, String> {
    hotdownloader_core::platforms::qqmusic::playlist::fetch_created_playlists(
        &TauriLoginStore::new(app),
    )
    .await
}

pub(crate) async fn fetch_created_playlist_songs(
    app: &AppHandle,
    separator: &str,
    id: String,
    dirid: String,
) -> Result<String, String> {
    hotdownloader_core::platforms::qqmusic::playlist::fetch_created_playlist_songs(
        &TauriLoginStore::new(app),
        separator,
        id,
        dirid,
    )
    .await
}
