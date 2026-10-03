//! QQ 音乐下载凭据端口及基于 JSON 文件的实现。
//! 文件字段沿用客户端历史设置中的 loginUin/authst/refreshToken 等名称，便于迁移已有登录态。

use std::path::{Path, PathBuf};

use futures_util::future::BoxFuture;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::Mutex;

/// 链接接口真正需要的两项凭据。令牌本身不进入任务记录或进度事件。
// 不派生 Debug，避免调用方在错误日志中意外输出 authst。
#[derive(Clone, PartialEq, Eq)]
pub struct QqAuth {
    pub uin: String,
    pub authst: String,
}

/// 用户在「设置 → 平台账号」里粘贴的 QQ 音乐网页版 Cookie。
///
/// QQ 音乐匿名取链通道已被平台关闭（见 [`crate::platforms::credentials`] 的实测记录），
/// 因此只有 `uin` + 登录密钥两件套齐全才拿得到直链。整条 Cookie 只在内存里解析一次，
/// 不落盘、不进日志。
// 与 [`QqAuth`] 同理不派生 Debug，避免错误路径意外打印登录密钥。
#[derive(Clone, PartialEq, Eq)]
pub struct QqCookie {
    uin: String,
    authst: String,
}

/// 登录密钥的候选键名，按优先级排列（新版网页键优先，旧键兜底）。
const AUTHST_KEYS: [&str; 4] = ["qqmusic_key", "qm_keyst", "qqmusic_key_2", "qq_music_key"];

impl QqCookie {
    /// 从整条 Cookie 解析出 `uin` 与登录密钥。
    ///
    /// 错误文案只提字段名与（截断后的）`uin` 取值，**不回显 Cookie 原文**，避免令牌进日志。
    pub fn parse(raw: &str) -> Result<Self, String> {
        let pairs = cookie_pairs(raw);

        // 网页版通常给 uin；微信登录态只带 wxuin，纯数字直接可用。
        let raw_uin = find_value(&pairs, &["uin"])
            .or_else(|| find_value(&pairs, &["wxuin"]))
            .ok_or_else(|| {
                "Cookie 里缺少 uin=...（QQ 音乐网页版 Cookie 里的 uin 字段）".to_string()
            })?;
        let uin = normalize_uin(raw_uin)?;
        let authst = find_value(&pairs, &AUTHST_KEYS).ok_or_else(|| {
            "Cookie 里缺少 qqmusic_key=... 或 qm_keyst=...（QQ 音乐网页版 Cookie 里的登录密钥）"
                .to_string()
        })?;

        Ok(Self {
            uin,
            authst: authst.to_string(),
        })
    }

    /// 取链接口认的数字 `uin`（已剥掉 `o`/`O` 前缀与前导零）。
    pub fn uin(&self) -> &str {
        &self.uin
    }

    /// 登录密钥（`qqmusic_key` / `qm_keyst` 等）。
    pub fn authst(&self) -> &str {
        &self.authst
    }

    /// 转成取链接口需要的凭据。
    pub fn into_auth(self) -> QqAuth {
        QqAuth {
            uin: self.uin,
            authst: self.authst,
        }
    }
}

/// 按 `;` 切分 Cookie：键名转小写去两端空白，忽略没有 `=` 或值为空的片段。
fn cookie_pairs(raw: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for pair in raw.split(';') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        pairs.push((key, value.to_string()));
    }
    pairs
}

/// 按 `names` 的优先级（而不是 Cookie 里的先后顺序）取第一个出现的值。
fn find_value<'a>(pairs: &'a [(String, String)], names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|name| {
        pairs
            .iter()
            .find(|(key, _)| key.as_str() == *name)
            .map(|(_, value)| value.as_str())
    })
}

/// 网页版给的 `uin` 形如 `o0123456789`（前缀 `o` + 前导零），而取链接口只认纯数字。
fn normalize_uin(raw: &str) -> Result<String, String> {
    let digits = raw
        .strip_prefix(['o', 'O'])
        .unwrap_or(raw)
        .trim_start_matches('0');
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("uin 不是纯数字：{}", truncate(raw, 32)));
    }
    Ok(digits.to_string())
}

/// 错误文案里最多带 32 个字符，避免把超长取值或整条 Cookie 写进日志。
fn truncate(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// 下载任务每次获取链接时读取当前登录态。安卓端可以从 Tauri Store 获取，
/// 基于文件的实现从本地文件获取；两者都无需让前端保管或传递令牌。
pub trait QqCredentialSource: Send + Sync {
    fn current(&self) -> BoxFuture<'_, Result<Option<QqAuth>, String>>;
}

/// 基于文件的凭据适配器。文件不存在代表匿名访问；格式损坏则明确报错，
/// 避免悄悄把带会员权限的下载改成匿名请求。
pub struct FileQqCredentialSource {
    path: PathBuf,
    client: Client,
    refresh_lock: Mutex<()>,
}

impl FileQqCredentialSource {
    pub fn new(path: impl Into<PathBuf>, client: Client) -> Self {
        Self {
            path: path.into(),
            client,
            refresh_lock: Mutex::new(()),
        }
    }

    async fn load_settings(&self) -> Result<Value, String> {
        let contents = match tokio::fs::read_to_string(&self.path).await {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
            Err(error) => return Err(format!("读取 QQ 凭据文件失败: {error}")),
        };
        let value: Value = serde_json::from_str(&contents)
            .map_err(|error| format!("解析 QQ 凭据文件失败: {error}"))?;
        if !value.is_object() {
            return Err("QQ 凭据文件必须是 JSON 对象".into());
        }
        Ok(value)
    }

    async fn save_settings(&self, settings: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("创建 QQ 凭据目录失败: {error}"))?;

        // Linux/Android 上用同目录临时文件与 rename 原子替换，进程中断时保留上一份完整凭据。
        let suffix = rand::random::<u64>();
        let name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("credentials");
        let temporary = parent.join(format!(".{name}.{suffix:x}.tmp"));
        tokio::fs::write(&temporary, bytes)
            .await
            .map_err(|error| format!("写入 QQ 凭据临时文件失败: {error}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
                .await
                .map_err(|error| format!("设置 QQ 凭据文件权限失败: {error}"))?;
        }

        // Windows 不支持覆盖已存在目标文件的 rename；此处退化为直接写入。
        #[cfg(windows)]
        if self.path.exists() {
            let contents = tokio::fs::read(&temporary)
                .await
                .map_err(|error| format!("读取 QQ 凭据临时文件失败: {error}"))?;
            tokio::fs::write(&self.path, contents)
                .await
                .map_err(|error| format!("更新 QQ 凭据文件失败: {error}"))?;
            let _ = tokio::fs::remove_file(&temporary).await;
            return Ok(());
        }

        tokio::fs::rename(&temporary, &self.path)
            .await
            .map_err(|error| format!("替换 QQ 凭据文件失败: {error}"))
    }

    async fn login_api_call(
        &self,
        module: &str,
        method: &str,
        param: Value,
        extra: Value,
    ) -> Result<Value, String> {
        let request_key = format!("{module}.{method}");
        let mut comm = json!({
            "ct": "11",
            "cv": "13020508",
            "v": "13020508",
            "tmeAppID": "qqmusic",
            "format": "json",
            "inCharset": "utf-8",
            "outCharset": "utf-8",
        });
        if let Some(fields) = extra.as_object() {
            for (key, value) in fields {
                comm[key] = value.clone();
            }
        }
        let request = json!({
            "comm": comm,
            request_key.clone(): {
                "module": module,
                "method": method,
                "param": param,
            }
        });
        let response = self
            .client
            .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
            .header("Referer", "https://y.qq.com/")
            .header("Origin", "https://y.qq.com")
            .json(&request)
            .send()
            .await
            .map_err(|error| format!("网络错误: {error}"))?;
        let body = response
            .text()
            .await
            .map_err(|error| format!("读取响应失败: {error}"))?;
        let data: Value =
            serde_json::from_str(&body).map_err(|error| format!("解析响应失败: {error}"))?;
        let response_data = data.get(&request_key).ok_or("响应缺少对应模块")?;
        if response_data["code"].as_i64().unwrap_or(-1) != 0 {
            return Err(format!("接口错误: code={}", response_data["code"]));
        }
        Ok(response_data["data"].clone())
    }

    async fn is_expired(&self, auth: &QqAuth) -> Result<bool, String> {
        match self
            .login_api_call(
                "music.UserInfo.userInfoServer",
                "GetLoginUserInfo",
                json!({}),
                json!({
                    "uin": auth.uin,
                    "authst": auth.authst,
                    "tmeLoginType": "6",
                }),
            )
            .await
        {
            Ok(_) => Ok(false),
            // 只有接口明确拒绝才认定过期。网络失败不能清除仍可能有效的令牌。
            Err(error) if error.starts_with("接口错误:") => Ok(true),
            Err(error) => Err(error),
        }
    }

    async fn refresh(&self, settings: &mut Value, old: &QqAuth) -> Result<QqAuth, String> {
        let refresh_token = settings["refreshToken"].as_str().unwrap_or("");
        let refresh_key = settings["refreshKey"].as_str().unwrap_or("");
        if refresh_token.is_empty() && refresh_key.is_empty() {
            return Err("缺少刷新令牌，无法自动刷新".into());
        }
        let param = build_refresh_param(settings, old)?;
        let data = self
            .login_api_call(
                "music.login.LoginServer",
                "Login",
                param,
                json!({ "tmeLoginType": "6" }),
            )
            .await?;
        let new_auth = QqAuth {
            uin: data["str_musicid"]
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| data["musicid"].as_u64().map(|id| id.to_string()))
                .ok_or("刷新响应缺少 musicid")?,
            authst: data["musickey"]
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or("刷新响应缺少 musickey")?
                .to_string(),
        };

        // 同一 JSON 中可能还有下载设置；刷新时只更新凭据字段，保留其他设置。
        settings["loginUin"] = json!(new_auth.uin);
        settings["authst"] = json!(new_auth.authst);
        for (source, target) in [
            ("refresh_token", "refreshToken"),
            ("refresh_key", "refreshKey"),
            ("access_token", "accessToken"),
            ("openid", "openid"),
        ] {
            if let Some(value) = data[source].as_str() {
                settings[target] = json!(value);
            }
        }
        settings["loginResponseData"] = data;
        self.save_settings(settings).await?;
        Ok(new_auth)
    }
}

/// 与客户端扫码登录使用相同的三种刷新参数；拆成纯函数便于离线验证。
fn build_refresh_param(settings: &Value, old: &QqAuth) -> Result<Value, String> {
    let music_id = old.uin.parse::<u64>().map_err(|_| "uin 必须为数字")?;
    let login_type = settings["loginResponseData"]["loginType"]
        .as_u64()
        .unwrap_or(0);
    let openid = settings["openid"].as_str().unwrap_or("");
    let access_token = settings["accessToken"].as_str().unwrap_or("");
    let refresh_token = settings["refreshToken"].as_str().unwrap_or("");
    let refresh_key = settings["refreshKey"].as_str().unwrap_or("");

    Ok(match login_type {
        1 => json!({
            "openid": openid, "refresh_token": refresh_token,
            "str_musicid": old.uin, "musickey": old.authst,
            "refresh_key": refresh_key, "loginMode": 2,
        }),
        2 => json!({
            "openid": openid, "access_token": access_token,
            "refresh_token": refresh_token, "expired_in": 0,
            "musicid": music_id, "musickey": old.authst,
            "refresh_key": refresh_key, "loginMode": 2,
        }),
        _ => json!({
            "openid": openid, "access_token": access_token,
            "refresh_token": refresh_token, "expired_in": 0,
            "str_musicid": old.uin, "musicid": music_id,
            "musickey": old.authst, "refresh_key": refresh_key,
            "loginMode": 2,
        }),
    })
}

impl QqCredentialSource for FileQqCredentialSource {
    fn current(&self) -> BoxFuture<'_, Result<Option<QqAuth>, String>> {
        Box::pin(async move {
            // 检查与刷新必须串行，否则并发下载可能同时使用一次性刷新令牌。
            let _guard = self.refresh_lock.lock().await;
            let mut settings = self.load_settings().await?;
            let auth = match (settings["loginUin"].as_str(), settings["authst"].as_str()) {
                (Some(uin), Some(authst)) if !uin.is_empty() && !authst.is_empty() => QqAuth {
                    uin: uin.to_string(),
                    authst: authst.to_string(),
                },
                _ => return Ok(None),
            };

            match self.is_expired(&auth).await {
                Ok(false) => Ok(Some(auth)),
                Err(error) => {
                    log::warn!("QQ 凭据校验失败，继续尝试现有凭据: {error}");
                    Ok(Some(auth))
                }
                Ok(true) => match self.refresh(&mut settings, &auth).await {
                    Ok(refreshed) => Ok(Some(refreshed)),
                    Err(error) => {
                        // 当前任务仍可尝试旧凭据；失败原因记录在后端日志，不广播令牌内容。
                        log::warn!("QQ 凭据刷新失败，继续尝试现有凭据: {error}");
                        Ok(Some(auth))
                    }
                },
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{build_refresh_param, FileQqCredentialSource, QqAuth, QqCredentialSource};
    use serde_json::json;

    #[tokio::test]
    async fn missing_and_invalid_files_are_distinguished() {
        let path = std::env::temp_dir().join(format!(
            "hotdownloader-qq-credentials-{:x}.json",
            rand::random::<u64>()
        ));
        let source = FileQqCredentialSource::new(&path, reqwest::Client::new());

        // 文件不存在表示尚未登录；损坏的 JSON 应显式失败，不能静默转匿名。
        assert!(source.current().await.unwrap().is_none());
        tokio::fs::write(&path, "{").await.unwrap();
        assert!(source
            .current()
            .await
            .err()
            .unwrap()
            .contains("解析 QQ 凭据文件失败"));
        tokio::fs::remove_file(&path).await.unwrap();
    }

    #[tokio::test]
    async fn refresh_save_keeps_unrelated_settings() {
        let path = std::env::temp_dir().join(format!(
            "hotdownloader-qq-credentials-{:x}.json",
            rand::random::<u64>()
        ));
        let source = FileQqCredentialSource::new(&path, reqwest::Client::new());
        let settings = json!({
            "downloadDir": "/music",
            "loginUin": "12345",
            "authst": "token",
        });

        source.save_settings(&settings).await.unwrap();
        assert_eq!(source.load_settings().await.unwrap(), settings);
        tokio::fs::remove_file(&path).await.unwrap();
    }

    #[test]
    fn refresh_parameters_follow_original_login_type() {
        let auth = QqAuth {
            uin: "12345".into(),
            authst: "current-token".into(),
        };
        let mut settings = json!({
            "refreshToken": "refresh-token",
            "refreshKey": "refresh-key",
            "accessToken": "access-token",
            "openid": "open-id",
            "loginResponseData": { "loginType": 1 },
        });

        let type_one = build_refresh_param(&settings, &auth).unwrap();
        assert_eq!(type_one["str_musicid"], "12345");
        assert!(type_one.get("musicid").is_none());

        settings["loginResponseData"]["loginType"] = json!(2);
        let type_two = build_refresh_param(&settings, &auth).unwrap();
        assert_eq!(type_two["musicid"], 12345);
        assert!(type_two.get("str_musicid").is_none());

        settings["loginResponseData"]["loginType"] = json!(0);
        let default_type = build_refresh_param(&settings, &auth).unwrap();
        assert_eq!(default_type["musicid"], 12345);
        assert_eq!(default_type["str_musicid"], "12345");
    }
}

/// `QqCookie` 的解析规则与错误文案。
///
/// 只覆盖解析层：合法 Cookie 一旦解析成功，取链会真的发网络请求
/// （`qqmusic::link::fetch_vkey_link` 没有前置参数校验），那种路径留给联网验证。
#[cfg(test)]
mod cookie_tests {
    use super::QqCookie;

    #[test]
    fn standard_web_cookie_is_parsed() {
        let raw =
            "pgv_pvid=1234; uin=o0123456789; qqmusic_key=KEY123; qm_keyst=OLD; qqmusic_key_2=K2";
        let cookie = QqCookie::parse(raw).unwrap();
        assert_eq!(cookie.uin(), "123456789");
        // 优先级：qqmusic_key > qm_keyst > qqmusic_key_2 > qq_music_key
        assert_eq!(cookie.authst(), "KEY123");
    }

    #[test]
    fn prefix_and_leading_zeros_are_stripped() {
        let cookie = QqCookie::parse("uin=o000007654321; qqmusic_key=k").unwrap();
        assert_eq!(cookie.uin(), "7654321");

        let cookie = QqCookie::parse("uin=O00042; qqmusic_key=k").unwrap();
        assert_eq!(cookie.uin(), "42");
    }

    #[test]
    fn wxuin_is_used_when_uin_is_absent() {
        let cookie = QqCookie::parse("wxuin=987654321; qqmusic_key=k").unwrap();
        assert_eq!(cookie.uin(), "987654321");

        // 两者都在时优先 uin。
        let cookie = QqCookie::parse("wxuin=111; uin=o222; qqmusic_key=k").unwrap();
        assert_eq!(cookie.uin(), "222");
    }

    #[test]
    fn legacy_and_alternate_authst_keys_are_accepted() {
        let cases = [
            ("uin=o42; qm_keyst=legacy", "legacy"),
            ("uin=o42; qqmusic_key_2=k2", "k2"),
            ("uin=o42; qq_music_key=k3", "k3"),
        ];
        for (raw, expected) in cases {
            let cookie = QqCookie::parse(raw).unwrap();
            assert_eq!(cookie.authst(), expected, "{raw}");
        }

        // 新版键压过旧键，即使旧键在 Cookie 里排在前面。
        let cookie = QqCookie::parse("uin=o42; qm_keyst=old; qqmusic_key=new").unwrap();
        assert_eq!(cookie.authst(), "new");
    }

    #[test]
    fn key_names_tolerate_case_and_spacing() {
        let cookie = QqCookie::parse("  UIN =  o0042 ;  QQMusic_Key =  k  ").unwrap();
        assert_eq!(cookie.uin(), "42");
        assert_eq!(cookie.authst(), "k");
    }

    #[test]
    fn missing_key_is_reported_without_echoing_the_cookie() {
        let raw = "uin=o0123456789; pgv_pvid=SECRETPVID";
        let error = QqCookie::parse(raw).err().expect("缺少登录密钥应当失败");
        assert!(error.contains("qqmusic_key"), "{error}");
        assert!(!error.contains("SECRETPVID"), "{error}");
    }

    #[test]
    fn missing_uin_is_reported_without_echoing_the_cookie() {
        let raw = "qqmusic_key=SECRETKEY; pgv_pvid=1";
        let error = QqCookie::parse(raw).err().expect("缺少 uin 应当失败");
        assert!(error.contains("uin"), "{error}");
        assert!(!error.contains("SECRETKEY"), "{error}");
    }

    #[test]
    fn non_numeric_uin_is_reported_truncated() {
        let error = QqCookie::parse("uin=oabcXYZ; qqmusic_key=k")
            .err()
            .expect("uin 非数字应当失败");
        assert_eq!(error, "uin 不是纯数字：oabcXYZ");

        let long = "x".repeat(40);
        let error = QqCookie::parse(&format!("uin={long}; qqmusic_key=k"))
            .err()
            .expect("uin 非数字应当失败");
        let shown: String = long.chars().take(32).collect();
        assert!(error.contains(&shown), "{error}");
        assert!(!error.contains(&long), "{error}");
    }

    #[test]
    fn into_auth_carries_the_normalized_uin() {
        let auth = QqCookie::parse("uin=o00042; qqmusic_key=k")
            .unwrap()
            .into_auth();
        assert_eq!(auth.uin, "42");
        assert_eq!(auth.authst, "k");
    }
}
