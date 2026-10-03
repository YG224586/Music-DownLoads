//! QQ 音乐下载凭据管理模块。
//!
//! 取链接口需要用户自己的 uin/authst 才能解锁更高音质或会员歌曲；本模块负责读取、
//! 校验与刷新这份凭据。旧版本的扫码登录（MQTT over WebSocket）与手动登录入口已按
//! 用户要求移除，凭据只从设置存储读取，并继续支持过期时用刷新令牌自动续期。
//! 参考实现：<https://github.com/L-1124/QQMusicApi/blob/108617ffe80abefec6358717b9f4d3677550db10/qqmusic_api/modules/login.py>

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::credentials::QqAuth;
use crate::platforms::CLIENT;

/// 凭据读写接口，不接触窗口、IPC 或 Android 文件接口。
/// 各运行时负责提供自己的存储；刷新策略留在核心。
pub trait LoginCredentialStore: Send + Sync {
    fn load(&self) -> Result<Value, String>;
    fn save(&self, settings: &Value) -> Result<(), String>;
}

/// 本地 JSON 凭据文件。与下载链接凭据源读取同一份文件。
pub struct FileLoginStore {
    path: PathBuf,
    write_lock: std::sync::Mutex<()>,
}

impl FileLoginStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write_lock: std::sync::Mutex::new(()),
        }
    }
}

impl LoginCredentialStore for FileLoginStore {
    fn load(&self) -> Result<Value, String> {
        let contents = match std::fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
            Err(error) => return Err(format!("读取 QQ 凭据文件失败: {error}")),
        };
        let settings: Value = serde_json::from_str(&contents)
            .map_err(|error| format!("解析 QQ 凭据文件失败: {error}"))?;
        if !settings.is_object() {
            return Err("QQ 凭据文件必须是 JSON 对象".into());
        }
        Ok(settings)
    }

    fn save(&self, settings: &Value) -> Result<(), String> {
        let _guard = self.write_lock.lock().map_err(|error| error.to_string())?;
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建 QQ 凭据目录失败: {error}"))?;
        let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
        let temporary = self
            .path
            .with_extension(format!("{:x}.tmp", rand::random::<u64>()));
        std::fs::write(&temporary, bytes)
            .map_err(|error| format!("写入 QQ 凭据临时文件失败: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("设置 QQ 凭据文件权限失败: {error}"))?;
        }
        // Windows 无法通过 rename 覆盖现有文件；其余平台使用同目录原子替换。
        #[cfg(windows)]
        if self.path.exists() {
            let result = std::fs::write(
                &self.path,
                std::fs::read(&temporary).map_err(|e| e.to_string())?,
            )
            .map_err(|error| format!("更新 QQ 凭据文件失败: {error}"));
            let _ = std::fs::remove_file(&temporary);
            return result;
        }
        std::fs::rename(&temporary, &self.path)
            .map_err(|error| format!("替换 QQ 凭据文件失败: {error}"))
    }
}

/// 存储中的凭据信息。
// 登录凭据不能派生 Debug，避免日志意外输出 authst 与刷新令牌。
#[derive(Clone)]
pub struct LoginCredentials {
    /// 用户 QQ 号。
    pub uin: String,
    /// 登录授权令牌（authst）。
    pub authst: String,
    /// 刷新令牌。
    pub refresh_token: String,
    /// 刷新密钥。
    pub refresh_key: String,
    /// 访问令牌。
    pub access_token: String,
    /// OpenID。
    pub openid: String,
    /// 完整的登录响应数据（原始 JSON）
    pub raw_data: Option<Value>,
}

/// `music.login.LoginServer.Login` 接口返回的数据结构。
#[derive(serde::Serialize, serde::Deserialize)]
struct TLoginInfoData {
    /// 音乐 ID（数字 uin）。
    musicid: u64,
    /// 音乐 Key。
    musickey: String,
    /// 刷新令牌。
    refresh_token: String,
    /// 刷新密钥。
    refresh_key: String,
    /// 访问令牌（可能为空）。
    #[serde(default)]
    access_token: String,
    /// OpenID（可能为空）。
    #[serde(default)]
    openid: String,
    /// 字符串形式的音乐 ID（可能不存在）。
    #[serde(default)]
    str_musicid: Option<String>,
    /// Key 过期时间（秒），暂未使用。
    #[allow(dead_code)]
    #[serde(rename = "keyExpiresIn", default)]
    key_expires_in: i64,
    /// Key 创建时间戳（秒），暂未使用。
    #[allow(dead_code)]
    #[serde(rename = "musickeyCreateTime", default)]
    musickey_create_time: i64,
}

impl TLoginInfoData {
    /// 将接口返回的数据转换为 [`LoginCredentials`]。
    ///
    /// 优先使用 `str_musicid` 作为 uin，如果缺失则使用数字 `musicid` 转换。
    fn into_credentials(self) -> LoginCredentials {
        // 将整个响应数据序列化为 JSON Value，保存到 raw_data 中，
        // 这样后续如果需要 loginType、unionid、str_musicid 等字段，可以直接从 raw_data 获取，
        // 无需为每个字段单独增加结构体成员。
        let raw_data = serde_json::to_value(&self).ok();

        let uin = self
            .str_musicid
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.musicid.to_string());
        LoginCredentials {
            uin,
            authst: self.musickey,
            refresh_token: self.refresh_token,
            refresh_key: self.refresh_key,
            access_token: self.access_token,
            openid: self.openid,
            raw_data,
        }
    }
}

/// 构造 QQ 音乐统一接口请求体。
///
/// # 参数
/// - `module`: 接口模块名。
/// - `method`: 接口方法名。
/// - `param`: 接口参数。
/// - `comm`: 公共参数（会合并到默认参数中）。
///
/// # 返回
/// 序列化为 JSON 值的请求体。
fn build_http_body(module: &str, method: &str, param: Value, comm: Value) -> Value {
    json!({
        "comm": comm,
        format!("{module}.{method}"): {
            "module": module,
            "method": method,
            "param": param,
        }
    })
}

/// 调用 QQ 音乐统一接口。
///
/// # 参数
/// - `module`: 接口模块名。
/// - `method`: 接口方法名。
/// - `param`: 接口参数。
/// - `comm_extra`: 额外的公共参数，会与默认参数合并。
///
/// # 返回
/// - `Ok(Value)`: 接口返回的 `data` 字段。
/// - `Err(String)`: 错误信息。
async fn login_api_call(
    module: &str,
    method: &str,
    param: Value,
    comm_extra: Value,
) -> Result<Value, String> {
    let req_key = format!("{module}.{method}");
    let mut comm = json!({
        "ct": "11",
        "cv": "13020508",
        "v": "13020508",
        "tmeAppID": "qqmusic",
        "format": "json",
        "inCharset": "utf-8",
        "outCharset": "utf-8",
    });
    if let Some(obj) = comm_extra.as_object() {
        for (k, v) in obj {
            comm[k] = v.clone();
        }
    }

    let body = build_http_body(module, method, param, comm);
    let resp = CLIENT
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Referer", "https://y.qq.com/")
        .header("Origin", "https://y.qq.com")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    let req_data = data.get(&req_key).ok_or("响应缺少对应模块")?;
    if req_data["code"].as_i64().unwrap_or(-1) != 0 {
        log::warn!(
            "[登录] API 调用失败: module={}, method={}, code={}",
            module,
            method,
            req_data["code"]
        );
        return Err(format!("接口错误: code={}", req_data["code"]));
    }
    log::debug!("[登录] API 调用成功: module={}, method={}", module, method);
    Ok(req_data["data"].clone())
}

/// 将登录凭据保存到应用设置存储中。
///
/// # 参数
/// - `store`: 运行时提供的登录凭据存储。
/// - `creds`: 登录凭据。
///
/// # 返回
/// - `Ok(())`: 保存成功。
/// - `Err(String)`: 错误信息。
async fn save_credentials(
    store: &dyn LoginCredentialStore,
    creds: &LoginCredentials,
) -> Result<(), String> {
    let mut settings = store.load()?;
    settings["loginUin"] = json!(creds.uin);
    settings["authst"] = json!(creds.authst);
    settings["refreshToken"] = json!(creds.refresh_token);
    settings["refreshKey"] = json!(creds.refresh_key);
    settings["accessToken"] = json!(creds.access_token);
    settings["openid"] = json!(creds.openid);
    // 保存完整的登录响应数据（若存在），以便后续按需解析 loginType 等字段
    settings["loginResponseData"] = creds.raw_data.clone().unwrap_or(json!({}));
    store.save(&settings)
}

/// 从应用设置中读取当前登录凭据的 uin 和 authst。
///
/// # 参数
/// - `store`: 运行时提供的登录凭据存储。
///
/// # 返回
/// 元组 `(uin, authst)`，若未登录则两者均为 `None`。
pub async fn get_login_credentials(
    store: &dyn LoginCredentialStore,
) -> (Option<String>, Option<String>) {
    let settings = store.load().unwrap_or(json!({}));
    let uin = settings
        .get("loginUin")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let authst = settings
        .get("authst")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    (uin, authst)
}

/// 查询当前登录状态（只读）。
///
/// 登录入口已按用户要求移除，因此这里只反映本地是否仍存有可用凭据：
/// 命令本身不接受也不写入任何凭据，仅用于判断是否还能取到会员/高音质链接。
///
/// # 返回
/// JSON 字符串 `{"logged_in": bool, "uin": string}`；未登录时 `uin` 为空串。
pub async fn get_login_status(store: &dyn LoginCredentialStore) -> Result<String, String> {
    let (uin, authst) = get_login_credentials(store).await;
    let logged_in = uin.is_some() && authst.is_some();
    let uin = if logged_in {
        uin.unwrap_or_default()
    } else {
        String::new()
    };
    Ok(json!({ "logged_in": logged_in, "uin": uin }).to_string())
}

/// 下载链接使用的凭据，以及需要由运行时提示用户的刷新错误。
/// 刷新失败仍返回旧凭据，使链接请求有机会继续成功。
pub struct ResolvedDownloadAuth {
    /// 当前可用于链接请求的凭据；未登录时为 `None`。
    pub auth: Option<QqAuth>,
    /// 仅在确认过期且刷新失败时有值；运行时可据此提示用户。
    pub refresh_error: Option<String>,
}

/// 读取当前登录态，并在凭据明确过期时尝试刷新。
/// 校验接口的网络或解析错误不代表凭据过期，继续使用原凭据。
pub async fn download_auth(store: &dyn LoginCredentialStore) -> ResolvedDownloadAuth {
    let (uin, authst) = get_login_credentials(store).await;
    let Some((uin, authst)) = uin.zip(authst) else {
        return ResolvedDownloadAuth {
            auth: None,
            refresh_error: None,
        };
    };
    let mut auth = QqAuth { uin, authst };

    let expired = match check_credential_expired(store).await {
        Ok(expired) => expired,
        Err(error) => {
            // 校验请求失败无法证明凭据过期，继续用旧凭据获取链接。
            log::warn!("QQ 凭据校验失败，继续使用现有凭据: {error}");
            false
        }
    };

    if expired {
        match refresh_credential(store).await {
            Ok(credentials) => {
                auth = QqAuth {
                    uin: credentials.uin,
                    authst: credentials.authst,
                };
            }
            Err(error) => {
                // 保留任务启动时读到的旧凭据；刷新错误交给运行时通知用户。
                return ResolvedDownloadAuth {
                    auth: Some(auth),
                    refresh_error: Some(error),
                };
            }
        }
    }

    ResolvedDownloadAuth {
        auth: Some(auth),
        refresh_error: None,
    }
}

/// 检查当前登录凭证是否已过期。
///
/// 通过调用 `music.UserInfo.userInfoServer.GetLoginUserInfo` 接口验证凭证有效性。
/// 若凭证不存在或接口返回非零 code（1000/104401/104400），则视为已过期。
pub async fn check_credential_expired(store: &dyn LoginCredentialStore) -> Result<bool, String> {
    let Some(creds) = read_full_credentials(store).await else {
        return Ok(true);
    };
    if creds.authst.is_empty() {
        return Ok(true);
    }

    let param = json!({});
    match login_api_call(
        "music.UserInfo.userInfoServer",
        "GetLoginUserInfo",
        param,
        json!({
            "uin": creds.uin,
            "authst": creds.authst,
            "tmeLoginType": "6",
        }),
    )
    .await
    {
        Ok(_) => Ok(false),
        Err(e) => {
            if e.starts_with("接口错误:") {
                // 接口明确返回错误码，视为凭证已过期
                log::warn!("[check_credential_expired] 凭证校验失败，视为已过期: {}", e);
                Ok(true)
            } else {
                // 网络错误、解析错误等，不应视为凭证过期，原样返回错误
                Err(e)
            }
        }
    }
}

/// 尝试刷新登录凭证。
///
/// 使用存储中的刷新令牌（refresh_token/refresh_key）调用 `music.login.LoginServer.Login` 接口获取新凭证，
/// 并更新到设置存储中。
pub async fn refresh_credential(
    store: &dyn LoginCredentialStore,
) -> Result<LoginCredentials, String> {
    // 读取完整凭证，若不存在则直接返回错误（无需清除，因为本来就没有）
    let creds = match read_full_credentials(store).await {
        Some(c) => c,
        None => return Err("未找到登录凭证".into()),
    };
    if creds.refresh_token.is_empty() && creds.refresh_key.is_empty() {
        // 缺少刷新令牌，无法刷新，清除当前凭证并返回错误
        let _ = logout(store).await;
        return Err("缺少刷新令牌，无法自动刷新".into());
    }

    // 执行刷新流程，任意步骤失败都会清除凭证
    let refresh_result = async {
        let music_id = creds.uin.parse::<u64>().map_err(|_| "uin 必须为数字")?;
        // 从原始响应数据中获取 loginType，若缺失则默认为 0（走通用分支）
        let login_type = creds
            .raw_data
            .as_ref()
            .and_then(|raw| raw.get("loginType"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        // 根据登录类型构造不同的刷新参数（参考 QQMusicApi 实现）。
        let param = match login_type {
            1 => json!({
                "openid": creds.openid,
                "refresh_token": creds.refresh_token,
                "str_musicid": creds.uin,
                "musickey": creds.authst,
                "refresh_key": creds.refresh_key,
                "loginMode": 2,
            }),
            2 => json!({
                "openid": creds.openid,
                "access_token": creds.access_token,
                "refresh_token": creds.refresh_token,
                "expired_in": 0,
                "musicid": music_id,
                "musickey": creds.authst,
                "refresh_key": creds.refresh_key,
                "loginMode": 2,
            }),
            _ => json!({
                "openid": creds.openid,
                "access_token": creds.access_token,
                "refresh_token": creds.refresh_token,
                "expired_in": 0,
                "str_musicid": creds.uin,
                "musicid": music_id,
                "musickey": creds.authst,
                "refresh_key": creds.refresh_key,
                "loginMode": 2,
            }),
        };
        let data = login_api_call(
            "music.login.LoginServer",
            "Login",
            param,
            json!({"tmeLoginType": "6"}),
        )
        .await?;

        let new_creds: LoginCredentials = serde_json::from_value::<TLoginInfoData>(data)
            .map(|d| d.into_credentials())
            .map_err(|e| format!("解析凭据失败: {e}"))?;

        // 保存新凭证
        save_credentials(store, &new_creds).await?;
        log::info!("[登录] 凭证刷新成功，uin = {}", new_creds.uin);
        Ok(new_creds)
    }
    .await;

    match refresh_result {
        Ok(new_creds) => Ok(new_creds),
        Err(e) => {
            // 刷新失败，清除存储的登录凭证，确保后续不再使用过期状态
            let _ = logout(store).await;
            log::warn!("[登录] 凭证刷新失败，已清除登录凭证: {}", e);
            Err(e)
        }
    }
}

/// 从应用设置中读取完整的登录凭据（包含刷新令牌等字段）。
async fn read_full_credentials(store: &dyn LoginCredentialStore) -> Option<LoginCredentials> {
    let settings = store.load().ok()?;
    let uin = settings.get("loginUin")?.as_str()?.to_string();
    let authst = settings.get("authst")?.as_str()?.to_string();
    let refresh_token = settings
        .get("refreshToken")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let refresh_key = settings
        .get("refreshKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let access_token = settings
        .get("accessToken")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let openid = settings
        .get("openid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    // 读取完整的原始响应数据（可能不存在，例如手动登录只保存了基础字段）
    let raw_data = settings.get("loginResponseData").cloned();
    Some(LoginCredentials {
        uin,
        authst,
        refresh_token,
        refresh_key,
        access_token,
        openid,
        raw_data,
    })
}

/// 清除所有登录相关设置（保留下载目录等非登录设置）。
///
/// # 参数
/// - `store`: 运行时提供的登录凭据存储。
///
/// # 返回
/// - `Ok(())`: 清除成功。
/// - `Err(String)`: 错误信息。
pub async fn logout(store: &dyn LoginCredentialStore) -> Result<(), String> {
    let mut settings = store.load()?;
    if let Some(obj) = settings.as_object_mut() {
        for key in [
            "loginUin",
            "authst",
            "refreshToken",
            "refreshKey",
            "accessToken",
            "openid",
            "loginResponseData",
        ] {
            obj.remove(key);
        }
    }
    store.save(&settings)
}

#[cfg(test)]
mod tests {
    use super::{get_login_credentials, logout, FileLoginStore, LoginCredentialStore};
    use serde_json::json;

    #[tokio::test]
    async fn file_store_keeps_non_login_settings_when_logging_out() {
        let path = std::env::temp_dir().join(format!(
            "hotdownloader-login-{:x}.json",
            rand::random::<u64>()
        ));
        let store = FileLoginStore::new(&path);
        store
            .save(&json!({
                "loginUin": "123",
                "authst": "secret",
                "downloadDir": "music"
            }))
            .unwrap();

        let (uin, authst) = get_login_credentials(&store).await;
        assert_eq!(uin.as_deref(), Some("123"));
        assert_eq!(authst.as_deref(), Some("secret"));
        logout(&store).await.unwrap();
        let settings = store.load().unwrap();
        assert!(settings.get("authst").is_none());
        assert_eq!(settings["downloadDir"], "music");

        std::fs::remove_file(path).unwrap();
    }
}
