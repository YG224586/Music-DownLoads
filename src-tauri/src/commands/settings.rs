//! Tauri 设置命令。字段验证和冲突判定位于共享核心，存储使用现有 data.json。

use hotdownloader_core::platforms::account_state::set_platform_account;
use hotdownloader_core::platforms::credentials::PlatformCookies;
use hotdownloader_core::platforms::Platform;
use hotdownloader_core::settings::patch::{
    apply_patch, snapshot, SettingsPatch, SettingsPatchError, SettingsScope, SettingsSnapshot,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{command, AppHandle, Emitter, Manager};

use crate::storage::store_wrapper;
use hotdownloader_core::download::engine::DownloadEngine;

// 覆盖完整的写盘、调度器更新和事件广播，保持同一进程内的修订顺序。
static PATCH_COMMAND_LOCK: Mutex<()> = Mutex::new(());

fn read_stored(raw: &str) -> Result<Value, String> {
    if raw.is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str(raw).map_err(|error| format!("解析设置失败: {error}"))
    }
}

/// 冲突包含后端当前快照，前端才能让用户逐字段选择保留哪一个值。
fn patch_error(error: SettingsPatchError) -> String {
    match error {
        SettingsPatchError::Invalid(message) => message,
        SettingsPatchError::Conflict { fields, snapshot } => json!({
            "error": "设置已被其他页面更新",
            "fields": fields,
            "snapshot": snapshot,
        })
        .to_string(),
    }
}

// store 的写盘是同步操作；命令以 async 暴露，避免在界面主线程执行。
#[command]
pub async fn get_settings_snapshot(app: AppHandle) -> Result<SettingsSnapshot, String> {
    let raw = store_wrapper::load_string(&app, "settings").map_err(|error| error.to_string())?;
    Ok(snapshot(&read_stored(&raw)?))
}

#[command]
pub async fn patch_settings(
    app: AppHandle,
    patch: SettingsPatch,
) -> Result<SettingsSnapshot, String> {
    let _guard = PATCH_COMMAND_LOCK
        .lock()
        .map_err(|_| "设置命令锁已损坏".to_string())?;
    let mut committed: Option<SettingsSnapshot> = None;
    let mut concurrency_changed = false;
    store_wrapper::update_settings(&app, |previous| {
        let current = read_stored(previous)?;
        let applied = apply_patch(&current, patch, SettingsScope::Tauri).map_err(patch_error)?;
        if applied
            .changed_fields
            .iter()
            .any(|field| field == "maxConcurrent")
        {
            concurrency_changed = true;
        }
        if !applied.changed_fields.is_empty() {
            committed = Some(applied.snapshot.clone());
        }
        Ok(applied.stored.to_string())
    })?;

    if concurrency_changed {
        // 两个命令可能在写盘后以相反顺序恢复执行，因此以当前持久化值更新调度器。
        let latest =
            store_wrapper::load_string(&app, "settings").map_err(|error| error.to_string())?;
        let max = read_stored(&latest)?["maxConcurrent"]
            .as_u64()
            .unwrap_or(3)
            .clamp(1, 32) as u32;
        app.state::<DownloadEngine>().set_concurrency(max);
    }
    if let Some(saved) = committed {
        // 其他窗口的设置页收到同一份快照；当前窗口也可用它确认落盘结果。
        let _ = app.emit("settings-updated", &saved);
        Ok(saved)
    } else {
        let raw =
            store_wrapper::load_string(&app, "settings").map_err(|error| error.to_string())?;
        Ok(snapshot(&read_stored(&raw)?))
    }
}

// ---------------------------------------------------------------------------
// 平台账号 Cookie（QQ 音乐 / 酷狗 / 网易云 / 咪咕）
//
// `platformCookies` 是核心 patch 白名单之外的字段，因此走一对专用命令；
// 它是 Cookie 唯一的进出通道：不进 `settings-updated` 事件、不进日志、不进任务记录。
// ---------------------------------------------------------------------------

const PLATFORM_COOKIES_KEY: &str = "platformCookies";

/// 设置页读写各平台账号 Cookie 的载荷，与核心 `PlatformCookies` 同形。
///
/// 键名与存储键固定为 `qq` / `kugou` / `netease` / `migu`，值是该平台网页版的
/// 原始 Cookie 头字符串（例如 `uin=o0123456789; qqmusic_key=xxx`）。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlatformCookiesPayload {
    pub qq: Option<String>,
    pub kugou: Option<String>,
    pub netease: Option<String>,
    pub migu: Option<String>,
}

impl PlatformCookiesPayload {
    /// 去掉首尾空白；空串与纯空白都按「未配置」处理，与核心归一化规则一致。
    fn normalized(self) -> Self {
        Self {
            qq: normalize_cookie(self.qq),
            kugou: normalize_cookie(self.kugou),
            netease: normalize_cookie(self.netease),
            migu: normalize_cookie(self.migu),
        }
    }

    /// 只写已配置的平台，避免在设置里留下空字段。
    fn to_stored(&self) -> Value {
        let mut stored = serde_json::Map::new();
        for (key, cookie) in [
            ("qq", &self.qq),
            ("kugou", &self.kugou),
            ("netease", &self.netease),
            ("migu", &self.migu),
        ] {
            if let Some(cookie) = cookie {
                stored.insert(key.into(), Value::String(cookie.clone()));
            }
        }
        Value::Object(stored)
    }

    /// 字段缺失、非字符串或 `platformCookies` 不是对象时，该平台按未配置处理。
    fn from_stored(stored: &Value) -> Self {
        let read = |key: &str| {
            stored
                .get(PLATFORM_COOKIES_KEY)
                .and_then(|cookies| cookies.get(key))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        Self {
            qq: read("qq"),
            kugou: read("kugou"),
            netease: read("netease"),
            migu: read("migu"),
        }
    }
}

fn normalize_cookie(value: Option<String>) -> Option<String> {
    value
        .map(|cookie| cookie.trim().to_owned())
        .filter(|cookie| !cookie.is_empty())
}

/// 设置命令与下载取链层共用的解析入口。
///
/// 账号设置坏掉只 `log::warn!` 不报错：解析失败一律返回全空的 [`PlatformCookies`]，
/// 让下载按各音源自身的匿名能力继续；日志与错误文案都不带 Cookie 内容。
pub fn platform_cookies_from_settings(raw: &str) -> PlatformCookies {
    if raw.trim().is_empty() {
        return PlatformCookies::default();
    }
    let stored: Value = match serde_json::from_str(raw) {
        Ok(stored) => stored,
        Err(error) => {
            log::warn!("设置 JSON 解析失败，平台账号按未配置继续下载: {error}");
            return PlatformCookies::default();
        }
    };
    match stored.get(PLATFORM_COOKIES_KEY) {
        None | Some(Value::Null) => PlatformCookies::default(),
        Some(value) if !value.is_object() => {
            log::warn!("设置 platformCookies 不是对象，平台账号按未配置继续下载");
            PlatformCookies::default()
        }
        Some(_) => {
            let cookies = PlatformCookiesPayload::from_stored(&stored);
            PlatformCookies {
                qq: cookies.qq,
                kugou: cookies.kugou,
                netease: cookies.netease,
                migu: cookies.migu,
            }
        }
    }
}

/// 把酷狗 / 网易云 / 咪咕「是否已配置账号」写入核心的进程级状态。
///
/// 音质档位由各平台 parser 依据这个状态声明（例如网易云匿名不声明无损档）。
/// 只传布尔，**不接触 Cookie 内容**，日志里也只出现布尔值。
///
/// QQ 音乐不进这里：核心 `bit_of(Platform::QqMusic)` 返回 0（空操作），
/// 且 QQ 的取链档位不依赖该状态，只需要 Cookie 本身在场。
fn apply_platform_account_state(kugou: bool, netease: bool, migu: bool) {
    set_platform_account(Platform::Kugou, kugou);
    set_platform_account(Platform::Netease, netease);
    set_platform_account(Platform::Migu, migu);
    log::debug!("平台账号状态: 酷狗={kugou} 网易云={netease} 咪咕={migu}");
}

/// 启动时从设置恢复平台账号状态；设置读不到或坏掉都按「未配置」处理，不影响启动。
pub fn restore_platform_accounts(app: &AppHandle) {
    let cookies = match store_wrapper::load_string(app, "settings") {
        Ok(raw) => platform_cookies_from_settings(&raw),
        Err(error) => {
            log::warn!("读取设置失败，平台账号按未配置处理: {error}");
            PlatformCookies::default()
        }
    };
    apply_platform_account_state(
        cookies.kugou().is_some(),
        cookies.netease().is_some(),
        cookies.migu().is_some(),
    );
}

/// 读取各平台账号 Cookie（原值），供设置页回显。
#[command]
pub async fn get_platform_cookies(app: AppHandle) -> Result<PlatformCookiesPayload, String> {
    let raw = store_wrapper::load_string(&app, "settings").map_err(|error| error.to_string())?;
    Ok(PlatformCookiesPayload::from_stored(&read_stored(&raw)?))
}

/// 整体替换各平台账号 Cookie：只改 `settings.platformCookies`，空值即清除该平台账号。
///
/// 与 `patch_settings` 共用同一把命令锁和同一个 store 写入入口，避免两处写盘互相覆盖。
#[command]
pub async fn set_platform_cookies(
    app: AppHandle,
    cookies: PlatformCookiesPayload,
) -> Result<PlatformCookiesPayload, String> {
    let _guard = PATCH_COMMAND_LOCK
        .lock()
        .map_err(|_| "设置命令锁已损坏".to_string())?;
    let normalized = cookies.normalized();
    let stored_cookies = normalized.to_stored();
    store_wrapper::update_settings(&app, move |previous| {
        let current = read_stored(previous)?;
        let mut next = current
            .as_object()
            .cloned()
            .ok_or_else(|| "设置必须是 JSON 对象".to_string())?;
        if stored_cookies
            .as_object()
            .is_some_and(|value| value.is_empty())
        {
            next.remove(PLATFORM_COOKIES_KEY);
        } else {
            next.insert(PLATFORM_COOKIES_KEY.into(), stored_cookies);
        }
        Ok(Value::Object(next).to_string())
    })?;
    // 只记录「已配置/未配置」，不记录 Cookie 内容。
    log::info!(
        "平台账号凭据已更新: QQ={} 酷狗={} 网易云={} 咪咕={}",
        normalized.qq.is_some(),
        normalized.kugou.is_some(),
        normalized.netease.is_some(),
        normalized.migu.is_some()
    );
    // 写盘成功后立刻同步账号状态，让设置页的音质档位与取链层看到同一份事实。
    apply_platform_account_state(
        normalized.kugou.is_some(),
        normalized.netease.is_some(),
        normalized.migu.is_some(),
    );
    Ok(normalized)
}
