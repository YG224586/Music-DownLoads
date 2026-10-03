//! 下载链接与解密密钥获取模块。
//!
//! 该模块提供统一的下载链接获取接口，替代旧的加密/非加密分离接口。
//! 核心函数 [`fetch_vkey_link`] 会根据文件扩展名判断是否需要解密密钥，
//! 并返回完整的下载 URL 和对应的密钥（非加密文件密钥为空）。
//! 凭据由调用方提供，HTTP 请求与响应解析与运行时无关，可独立复用。

use reqwest::Client;
use serde_json::{json, Value};

use crate::platforms::qqmusic::credentials::QqAuth;

/// 保持原客户端的随机 GUID 格式，避免多个任务共享固定设备标识。
fn get_guid() -> String {
    const CHARSET: &[u8] = b"ABCDEF1234567890";
    use rand::Rng;
    let mut rng = rand::rng();
    (0..32)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect()
}

/// 统一获取下载链接与解密密钥（新接口：`vkey.GetVkeyServer.CgiGetVkey`）。
///
/// 替换旧的加密/非加密分离接口，统一使用一个接口获取所有品质的下载链接。
/// 登录态通过 `comm` 和 `param` 中的 `uin` 字段传递，未登录时为空字符串。
/// 响应中的 `sip` 数组为优先 CDN 列表，若不为空则使用第一个作为下载 URL 前缀，
/// 否则使用默认 CDN `https://wx.music.tc.qq.com/`。
/// `purl` 为不带 CDN 的相对路径，`ekey` 为解密密钥（可能为空）。
/// 最终返回值只对加密格式保留 `ekey`，普通音频的密钥返回空字符串。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-source/blob/55eb9881dad6ca895505352f3a0a7d1dfa3444e0/src/apis/tx.js#L42>
///
/// # 参数
/// - `song_mid`: 歌曲的唯一标识（mid）。
/// - `filename`: 品质文件名（例如 `M800001abc.mp3`），决定下载的具体文件。
/// - `credentials`: 可选登录态，匿名访问时传 `None`；登录态本身只影响
///   `result=104003` 这一档拒绝码的提示文案（匿名时请用户去填账号，已登录时说明权益不足）。
///
/// # 返回
/// - `Ok((String, String))`：元组 `(完整下载链接, 解密密钥)`。
///   普通音频的解密密钥为空。
/// - `Err(String)`：错误信息，包括网络错误、接口错误、文件不可下载等。
pub async fn fetch_vkey_link(
    client: &Client,
    song_mid: &str,
    filename: &str,
    credentials: Option<&QqAuth>,
) -> Result<(String, String), String> {
    let uin = credentials.map(|value| value.uin.as_str());
    let authst = credentials.map(|value| value.authst.as_str());
    // 构造请求体，comm 和 param 中携带登录态，guid 动态生成
    let request_body = json!({
        "loginUin": uin.unwrap_or(""),
        "comm": {
            "format": "json",
            "ct": 24,
            "cv": 0,
            "tmeAppID": "qqmusic",
            "uin": uin.unwrap_or(""),
            "qq": uin.unwrap_or(""),
            "authst": authst.unwrap_or("")
        },
        "vkey.GetVkeyServer.CgiGetVkey": {
            "module": "vkey.GetVkeyServer",
            "method": "CgiGetVkey",
            "param": {
                "guid": get_guid(),
                "filename": [filename],
                "songmid": [song_mid],
                "songtype": [0],
                "uin": uin.unwrap_or(""),
                "loginflag": 1,
                "platform": "20"
            }
        }
    });

    // 发送 POST 请求到统一接口
    let resp = client
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    parse_vkey_response(&data, filename, credentials.is_some())
}

/// 单独解析响应，便于验证 CDN 选择、拒绝错误和明文/加密格式的密钥边界。
///
/// `logged_in` 只用于选择 `result=104003` 的文案：匿名时让用户去
/// 「设置 → 平台账号」填 QQ 音乐 Cookie，已登录时说明当前账号没有该曲权益。
fn parse_vkey_response(
    data: &Value,
    filename: &str,
    logged_in: bool,
) -> Result<(String, String), String> {
    // 提取 vkey.GetVkeyServer.CgiGetVkey 子响应
    let vkey_resp = &data["vkey.GetVkeyServer.CgiGetVkey"];
    if vkey_resp["code"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!("CgiGetVkey 错误: code={}", vkey_resp["code"]));
    }

    // 提取 midurlinfo 数组
    let midurlinfo = vkey_resp["data"]["midurlinfo"]
        .as_array()
        .ok_or("缺少 midurlinfo")?;
    let item = midurlinfo.first().ok_or("未获取到文件信息")?;

    // 提取 purl 和 ekey
    let purl = item["purl"].as_str().unwrap_or("");
    let ekey = item["ekey"].as_str().unwrap_or("").to_string();

    // 检查 purl 是否为空或 result 是否非0
    let result_code = item["result"].as_i64().unwrap_or(0);
    if purl.is_empty() || result_code != 0 {
        let err_msg = match result_code {
            104003 => result_104003_message(logged_in),
            104004 => "该歌曲已下架或禁止下载".to_string(),
            _ => format!(
                "获取下载链接失败，错误码: {}，详情: {:?}",
                result_code,
                item["tips"].as_str().unwrap_or("")
            ),
        };
        return Err(err_msg);
    }

    // 确定下载 URL 前缀：优先使用响应中的 sip 列表第一个地址，若为空则使用默认 CDN
    let default_cdn = "https://wx.music.tc.qq.com/";
    let cdn_prefix = vkey_resp["data"]["sip"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(default_cdn);

    // 拼接完整下载 URL（使用选定的 CDN 前缀 + 相对路径）
    let full_url = format!("{}{}", cdn_prefix, purl);
    // 明文品质不应将接口中的 ekey 传给下载器，否则可能错误进入解密分支。
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if extension.eq_ignore_ascii_case("mgg") || extension.eq_ignore_ascii_case("mflac") {
        Ok((full_url, ekey))
    } else {
        Ok((full_url, String::new()))
    }
}

/// `result=104003` 是平台对「匿名请求」与「账号无该曲权益」共用的一档拒绝码。
/// 两种情形用户接下来要做的事完全不同，因此按登录态给两档可执行文案。
fn result_104003_message(logged_in: bool) -> String {
    if logged_in {
        "QQ 音乐：当前 QQ 音乐账号没有该曲目的下载权限（付费/VIP 曲目），请确认账号已开通权益且 Cookie 未过期".to_string()
    } else {
        "QQ 音乐：该歌曲需要登录 QQ 音乐账号，匿名请求被平台拒绝（result=104003），请在「设置 → 平台账号」填入 QQ 音乐 Cookie 后重试".to_string()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_vkey_response;

    #[test]
    fn only_encrypted_formats_receive_ekey() {
        let response = json!({
            "vkey.GetVkeyServer.CgiGetVkey": {
                "code": 0,
                "data": {
                    "sip": ["https://cdn.example/"],
                    "midurlinfo": [{ "purl": "audio", "ekey": "secret", "result": 0 }]
                }
            }
        });
        assert_eq!(
            parse_vkey_response(&response, "song.mp3", false).unwrap(),
            ("https://cdn.example/audio".into(), String::new())
        );
        assert_eq!(
            parse_vkey_response(&response, "song.mflac", false).unwrap(),
            ("https://cdn.example/audio".into(), "secret".into())
        );
    }

    #[test]
    fn platform_rejection_keeps_existing_error_contract() {
        let response = json!({
            "vkey.GetVkeyServer.CgiGetVkey": {
                "code": 0,
                "data": {
                    "midurlinfo": [{ "purl": "", "result": 104004 }]
                }
            }
        });
        // 104004（已下架/禁止下载）与登录态无关，两种情形文案一致。
        assert_eq!(
            parse_vkey_response(&response, "song.mp3", false).unwrap_err(),
            "该歌曲已下架或禁止下载"
        );
        assert_eq!(
            parse_vkey_response(&response, "song.mp3", true).unwrap_err(),
            "该歌曲已下架或禁止下载"
        );
    }

    #[test]
    fn anonymous_104003_tells_the_user_where_to_fill_the_account() {
        let response = json!({
            "vkey.GetVkeyServer.CgiGetVkey": {
                "code": 0,
                "data": {
                    "midurlinfo": [{ "purl": "", "result": 104003 }]
                }
            }
        });
        let error = parse_vkey_response(&response, "song.mp3", false).unwrap_err();
        assert!(error.contains("设置 → 平台账号"), "{error}");
        assert!(error.contains("QQ 音乐"), "{error}");
        assert!(error.contains("104003"), "{error}");
        assert!(!error.contains("其它音源"), "{error}");
    }

    #[test]
    fn logged_in_104003_reports_missing_entitlement() {
        let response = json!({
            "vkey.GetVkeyServer.CgiGetVkey": {
                "code": 0,
                "data": {
                    "midurlinfo": [{ "purl": "", "result": 104003 }]
                }
            }
        });
        let error = parse_vkey_response(&response, "song.mp3", true).unwrap_err();
        assert!(error.contains("没有该曲目的下载权限"), "{error}");
        assert!(!error.contains("设置 → 平台账号"), "{error}");
    }
}
