//! 网易云取直链：匿名走明文 `enhance/player/url`，配置账号后走 `enhance/player/url/v1`。
//!
//! 匿名（未登录）能力边界由 `_dev/probe/netease-probe.mjs` 实测确定：
//! - `br=128000` → `level=standard`，实际字节数与详情 `lMusic.size` 一致，完整曲目；
//! - `br=320000` → `level=exhigh`，实际字节数与详情 `hMusic.size` 一致，完整曲目；
//! - `br=999000`（无损）→ 静默降级为 `br=320000`，所以匿名不提供 flac/hires；
//! - 付费曲目（`fee=1`/`4`）只返回试听片段（实测 720813 字节，详情 `hMusic` 为 9526125），
//!   由 `freeTrialInfo` 判定并确定性失败，绝不把片段当完整歌曲交付。
//!
//! 登录态能力（v1 接口，`level` 由档位映射，`Cookie` 由调用方提供）：
//! - 只有带上账号自己的 Cookie，网易云才会按该账号的权益签发完整直链，
//!   包括 VIP 曲目与无损档；
//! - 账号无该档位权益时 `url` 为空或只给试听片段（`freeTrialInfo` 非 null），
//!   这些都按「没拿到」处理，随后仍按本平台匿名能力取链；
//! - 两条路都没拿到时给出本平台自己的确定性文案（见 [`PAID_RESTRICTED_MESSAGE`] 与
//!   [`ACCOUNT_NO_PERMISSION_MESSAGE`]），不会把这首曲子交给别的音源。

use reqwest::{Client, Url};
use serde_json::Value;

const PLAYER_URL: &str = "https://music.163.com/api/song/enhance/player/url";

/// 登录态取链接口：按 `level` 协商档位，由网易云按账号权益签发直链。
const PLAYER_URL_V1: &str = "https://music.163.com/api/song/enhance/player/url/v1";

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const COOKIE: &str = "appver=8.7.01; os=pc";

/// 未配置账号时的确定性失败文案。
pub const PAID_RESTRICTED_MESSAGE: &str =
    "网易云音乐：该歌曲为付费/VIP 曲目，需要网易云会员账号，请在「设置 → 平台账号」填入网易云 Cookie 后重试";

/// 已配置账号却没有该曲目（或该档位）权益时的确定性失败文案。
pub const ACCOUNT_NO_PERMISSION_MESSAGE: &str =
    "网易云音乐：当前网易云账号没有该曲目的下载权限（付费/会员曲目），请确认账号已开通权益且 Cookie 未过期";

/// 取得 `(直链, 解密密钥)`。网易云返回的是明文 mp3/flac，密钥恒为空串。
///
/// # 参数
/// - `client`：运行时复用的 HTTP 客户端。
/// - `song_mid`：`{歌曲 id}`（可能带 `|` 后缀，见 [`parse_song_id`]）。
/// - `filename`：`{码率}.{扩展名}`（`128.mp3` / `320.mp3` / `2000.flac`）。
/// - `cookie`：本机保存的网易云账号 Cookie；`None` / 空白串 = 按匿名能力取链。
pub async fn get_download_link(
    client: &Client,
    song_mid: &str,
    filename: &str,
    cookie: Option<&str>,
) -> Result<(String, String), String> {
    let song_id = parse_song_id(song_mid)?;
    let (bitrate, format) = parse_quality_filename(filename)?;
    let cookie = cookie.map(str::trim).filter(|value| !value.is_empty());

    // 1) 配置了账号：走 v1 接口，由网易云按该账号的权益签发直链（可解锁 VIP 曲目与无损）。
    let login = cookie.and_then(|raw_cookie| {
        login_level(bitrate, &format).map(|(level, extension)| (raw_cookie, level, extension))
    });
    if let Some((raw_cookie, level, extension)) = login {
        match fetch_login_entry(client, &song_id, level, &extension, raw_cookie).await {
            Ok(Some(url)) => return Ok((url, String::new())),
            Ok(None) => log::debug!("网易云账号未返回该档位的完整直链，改按匿名能力取链"),
            Err(error) => log::debug!("网易云登录态取链未成功（{error}），改按匿名能力取链"),
        }
    }

    // 2) 没配置账号（或账号通道没拿到）：按网易云自身的匿名能力取链。
    if format != "mp3" {
        return Err(if cookie.is_some() {
            // 账号通道已经试过且没拿到无损：给「查权益/查有效期」的结论。
            ACCOUNT_NO_PERMISSION_MESSAGE.to_string()
        } else {
            format!("网易云匿名访问仅提供 mp3 音质，{format} 需要登录会员后才能获取")
        });
    }

    let requested_br = bitrate * 1000;
    let data = player_url_request(client, &song_id, requested_br).await?;

    let entry = data
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| "网易云未返回音频信息（该曲目可能已下架）".to_string())?;

    let url = entry
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();

    if url.is_empty() {
        // 匿名通道里 `url` 为空有三种成因：无版权、已下架，或者这首是付费/VIP 曲目
        // （网易云对未登录请求直接不给地址，`br` 也是 0）。后一种正是用户要走
        // 「填账号」的情形，所以文案必须给出下一步动作，不能只说「需要登录」。
        return Err(empty_url_message(cookie.is_some()));
    }

    if !entry
        .get("freeTrialInfo")
        .map(Value::is_null)
        .unwrap_or(true)
    {
        return Err(restricted_message(cookie.is_some()));
    }

    let actual_br = entry.get("br").and_then(Value::as_u64).unwrap_or(0);
    if actual_br != u64::from(requested_br) {
        return Err(if cookie.is_some() {
            ACCOUNT_NO_PERMISSION_MESSAGE.to_string()
        } else {
            format!(
                "网易云未提供所选音质（请求 {requested_br}，实际返回 {actual_br}），未登录仅能获取标准音质"
            )
        });
    }

    Ok((url, String::new()))
}

/// 最终失败文案：同一个「拿不到」在有无账号时指向完全不同的下一步动作。
///
/// 没配账号 → 去设置页填 Cookie；配了账号 → 查权益是否开通、Cookie 是否过期。
fn restricted_message(has_account: bool) -> String {
    if has_account {
        ACCOUNT_NO_PERMISSION_MESSAGE.to_string()
    } else {
        PAID_RESTRICTED_MESSAGE.to_string()
    }
}

/// 匿名接口回空 `url` 时的文案：无版权/已下架/付费曲目都可能落到这里，
/// 但只有付费曲目能靠填账号解决，所以未配置账号时要把设置页路径写出来。
fn empty_url_message(has_account: bool) -> String {
    if has_account {
        ACCOUNT_NO_PERMISSION_MESSAGE.to_string()
    } else {
        "网易云音乐：未返回可用的音频直链（该曲目可能无版权、已下架，或为付费/VIP 曲目需要登录）——若确为付费/VIP 曲目，请在「设置 → 平台账号」填入网易云 Cookie 后重试"
            .to_string()
    }
}

/// 档位 → v1 接口的 `level` 与 `encodeType`。
///
/// 只映射实测过的三档；其它组合（例如 `192.mp3`）没有对应的 `level`，仍按匿名判定处理。
fn login_level(bitrate: u32, format: &str) -> Option<(&'static str, String)> {
    let level = match (bitrate, format) {
        (128, "mp3") => "standard",
        (320, "mp3") => "exhigh",
        (2000, "flac") => "lossless",
        _ => return None,
    };
    Some((level, format.to_string()))
}

/// 构造 v1 请求地址：`ids` 是 JSON 数组字面量，`level`/`encodeType` 决定档位与容器。
fn player_v1_url(song_id: &str, level: &str, extension: &str) -> Result<Url, String> {
    Url::parse_with_params(
        PLAYER_URL_V1,
        &[
            ("ids", format!("[{song_id}]")),
            ("level", level.to_string()),
            ("encodeType", extension.to_string()),
        ],
    )
    .map_err(|error| format!("URL 构建失败: {error}"))
}

/// 登录态请求头里的 Cookie：保留调用方整条 Cookie，只在缺少 `os` 时补 `; os=pc`。
///
/// 网易云用 `os=pc` 判定网页端来源；重复追加同名键会让部分网关把请求判成非法。
fn login_cookie_header(raw_cookie: &str) -> String {
    let cookie = raw_cookie.trim();
    let has_os = cookie.split(';').any(|part| {
        part.split_once('=')
            .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("os"))
    });
    if has_os {
        cookie.to_string()
    } else {
        format!("{cookie}; os=pc")
    }
}

/// 登录态取链：请求 v1 接口并抽出该账号权益内的直链。
///
/// - `Ok(Some(url))`：拿到了完整曲目（含容器校验通过）的直链；
/// - `Ok(None)`：账号没有该档位权益（`url` 为空、只给试听片段，或容器与请求档位不符）；
/// - `Err(String)`：网络 / 接口层面失败，仅供日志诊断，不进入用户可见文案。
///
/// Cookie 只作为请求头发送，不写日志、不进入错误文案。
async fn fetch_login_entry(
    client: &Client,
    song_id: &str,
    level: &str,
    extension: &str,
    raw_cookie: &str,
) -> Result<Option<String>, String> {
    let url = player_v1_url(song_id, level, extension)?;
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::REFERER, "https://music.163.com/")
        .header(reqwest::header::COOKIE, login_cookie_header(raw_cookie))
        .send()
        .await
        .map_err(|error| format!("网络错误: 网易云取链请求失败: {error}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取响应失败: 网易云取链响应读取失败: {error}"))?;

    let parsed: Value = serde_json::from_str(&text)
        .map_err(|error| format!("解析响应失败: 网易云取链响应解析失败: {error}"))?;

    if !status.is_success() {
        return Err(format!("网易云取链接口返回 HTTP {status}"));
    }

    let code = parsed.get("code").and_then(Value::as_i64).unwrap_or(0);
    if code != 200 {
        let message = parsed
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(format!("网易云接口错误: code={code}, message={message}"));
    }

    let entry = parsed
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first());
    Ok(entry.and_then(|entry| login_entry_url(entry, extension)))
}

/// 登录态成功判定：`url` 非空、`freeTrialInfo` 为 null，且容器（`type`）与请求档位一致。
///
/// `freeTrialInfo` 缺失按「非试听」处理，与匿名路径的既有判定保持一致。
fn login_entry_url(entry: &Value, expected_extension: &str) -> Option<String> {
    let url = entry
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if url.is_empty() {
        return None;
    }
    if !entry
        .get("freeTrialInfo")
        .map(Value::is_null)
        .unwrap_or(true)
    {
        return None;
    }
    if let Some(kind) = entry.get("type").and_then(Value::as_str) {
        if !kind.is_empty() && !kind.eq_ignore_ascii_case(expected_extension) {
            return None;
        }
    }
    Some(url.to_string())
}

async fn player_url_request(client: &Client, song_id: &str, br: u32) -> Result<Value, String> {
    let response = client
        .get(PLAYER_URL)
        .query(&[("ids", format!("[{song_id}]")), ("br", br.to_string())])
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::REFERER, "https://music.163.com/")
        .header(reqwest::header::COOKIE, COOKIE)
        .send()
        .await
        .map_err(|error| format!("网络错误: 网易云取链请求失败: {error}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取响应失败: 网易云取链响应读取失败: {error}"))?;

    let parsed: Value = serde_json::from_str(&text)
        .map_err(|error| format!("解析响应失败: 网易云取链响应解析失败: {error}"))?;

    if !status.is_success() {
        return Err(format!("网易云取链接口返回 HTTP {status}"));
    }

    let code = parsed.get("code").and_then(Value::as_i64).unwrap_or(0);
    if code != 200 {
        let message = parsed
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(format!("网易云接口错误: code={code}, message={message}"));
    }

    Ok(parsed)
}

/// 解析网易云歌曲 ID；核心下载层也会用它反查歌曲详情。
pub(crate) fn parse_song_id(song_mid: &str) -> Result<String, String> {
    let id = song_mid.split('|').next().unwrap_or_default().trim();

    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("网易云歌曲 ID 无效: {song_mid}"));
    }

    Ok(id.to_string())
}

fn parse_quality_filename(filename: &str) -> Result<(u32, String), String> {
    let (stem, format) = filename
        .rsplit_once('.')
        .ok_or_else(|| format!("网易云音质文件名缺少扩展名: {filename}"))?;

    let bitrate = stem
        .parse::<u32>()
        .map_err(|_| format!("网易云音质文件名缺少码率: {filename}"))?;

    if bitrate == 0 || format.is_empty() {
        return Err(format!("网易云音质文件名无效: {filename}"));
    }

    Ok((bitrate, format.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::{
        empty_url_message, login_cookie_header, login_entry_url, login_level,
        parse_quality_filename, parse_song_id, player_v1_url, restricted_message,
        ACCOUNT_NO_PERMISSION_MESSAGE, PAID_RESTRICTED_MESSAGE,
    };
    use serde_json::json;

    #[test]
    fn quality_filename_follows_task_contract() {
        assert_eq!(
            parse_quality_filename("128.mp3").unwrap(),
            (128, "mp3".to_string())
        );
        assert_eq!(
            parse_quality_filename("320.mp3").unwrap(),
            (320, "mp3".to_string())
        );
        assert!(parse_quality_filename("mp3").is_err());
        assert!(parse_quality_filename("flac.mp3").is_err());
    }

    #[test]
    fn song_id_rejects_invalid_value() {
        assert_eq!(parse_song_id("2652820720").unwrap(), "2652820720");
        assert!(parse_song_id("").is_err());
        assert!(parse_song_id("abc").is_err());
    }

    #[test]
    fn login_level_maps_only_measured_tiers() {
        assert_eq!(
            login_level(128, "mp3"),
            Some(("standard", "mp3".to_string()))
        );
        assert_eq!(login_level(320, "mp3"), Some(("exhigh", "mp3".to_string())));
        assert_eq!(
            login_level(2000, "flac"),
            Some(("lossless", "flac".to_string()))
        );
        // 没有对应 level 的组合仍按匿名判定处理
        assert_eq!(login_level(192, "mp3"), None);
        assert_eq!(login_level(128, "flac"), None);
        assert_eq!(login_level(2000, "mp3"), None);
    }

    #[test]
    fn player_v1_url_carries_id_level_and_extension() {
        let url = player_v1_url("2652820720", "exhigh", "mp3").unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("music.163.com"));
        assert_eq!(url.path(), "/api/song/enhance/player/url/v1");

        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        for expected in [
            // ids 是 JSON 数组字面量（query_pairs 已解码，故这里看到的是原文）
            ("ids", "[2652820720]"),
            ("level", "exhigh"),
            ("encodeType", "mp3"),
        ] {
            assert!(
                pairs
                    .iter()
                    .any(|(key, value)| key == expected.0 && value == expected.1),
                "缺少参数 {}={}：{:?}",
                expected.0,
                expected.1,
                pairs
            );
        }

        let lossless = player_v1_url("186016", "lossless", "flac").unwrap();
        let lossless_pairs: Vec<(String, String)> = lossless
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        assert!(lossless_pairs
            .iter()
            .any(|(key, value)| key == "level" && value == "lossless"));
        assert!(lossless_pairs
            .iter()
            .any(|(key, value)| key == "encodeType" && value == "flac"));
    }

    #[test]
    fn login_cookie_header_keeps_user_cookie_and_adds_os_pc_once() {
        // 用户 Cookie 原样保留，缺 os 时补一份
        assert_eq!(
            login_cookie_header("MUSIC_U=abc; __csrf=xyz"),
            "MUSIC_U=abc; __csrf=xyz; os=pc"
        );
        // 已含 os=pc 时不重复
        let with_os = login_cookie_header("MUSIC_U=abc; os=pc");
        assert_eq!(with_os, "MUSIC_U=abc; os=pc");
        assert_eq!(with_os.matches("os=").count(), 1);
        // 键名大小写不敏感，同样不重复
        let upper = login_cookie_header("MUSIC_U=abc; OS=pc");
        assert_eq!(upper, "MUSIC_U=abc; OS=pc");
        assert_eq!(upper.to_ascii_lowercase().matches("os=").count(), 1);
        // 脏输入（前后空白、空片段）也能处理
        assert_eq!(login_cookie_header("  MUSIC_U=abc  "), "MUSIC_U=abc; os=pc");
        // Cookie 里的音乐 U 值不因补键被改动
        assert!(login_cookie_header("MUSIC_U=abc").starts_with("MUSIC_U=abc"));
    }

    #[test]
    fn login_entry_url_requires_full_track_and_matching_container() {
        // 实测成功形态：url 非空 + freeTrialInfo 为 null
        assert_eq!(
            login_entry_url(
                &json!({
                    "url": "https://m7.music.126.net/20261004/abc.mp3",
                    "freeTrialInfo": null,
                    "br": 320000,
                    "type": "mp3",
                    "level": "exhigh",
                }),
                "mp3",
            )
            .as_deref(),
            Some("https://m7.music.126.net/20261004/abc.mp3")
        );

        // 只给试听片段 ⇒ 不算成功
        assert!(login_entry_url(
            &json!({
                "url": "https://m7.music.126.net/20261004/trial.mp3",
                "freeTrialInfo": { "start": 0, "end": 30 },
                "type": "mp3",
            }),
            "mp3",
        )
        .is_none());

        // 账号无该档位权益时 url 为空 ⇒ 不算成功
        assert!(login_entry_url(&json!({ "url": "", "freeTrialInfo": null }), "mp3").is_none());
        assert!(login_entry_url(&json!({ "freeTrialInfo": null }), "mp3").is_none());

        // 容器与请求档位不符 ⇒ 不算成功（不能让扩展名说谎）
        assert!(login_entry_url(
            &json!({ "url": "https://m7.music.126.net/a.flac", "type": "flac" }),
            "mp3",
        )
        .is_none());
        // 缺 type / freeTrialInfo 时按宽松判定（与匿名路径一致）
        assert!(
            login_entry_url(&json!({ "url": "https://m7.music.126.net/a.mp3" }), "mp3").is_some()
        );
        assert!(
            login_entry_url(&json!({ "url": "https://m7.music.126.net/a.flac" }), "flac").is_some()
        );
        // 前后空白要清掉
        assert_eq!(
            login_entry_url(
                &json!({ "url": "  https://m7.music.126.net/a.mp3  ", "freeTrialInfo": null }),
                "mp3",
            )
            .as_deref(),
            Some("https://m7.music.126.net/a.mp3")
        );
    }

    #[test]
    fn restricted_message_depends_on_configured_account() {
        let anonymous = restricted_message(false);
        assert_eq!(anonymous, PAID_RESTRICTED_MESSAGE);
        assert!(
            anonymous.contains("设置 → 平台账号"),
            "未配置账号的文案必须给出落地页：{anonymous}"
        );
        assert!(anonymous.contains("网易云"), "{anonymous}");

        let configured = restricted_message(true);
        assert_eq!(configured, ACCOUNT_NO_PERMISSION_MESSAGE);
        assert!(
            configured.contains("权益") && configured.contains("未过期"),
            "已配置账号的文案必须指向权益与有效期：{configured}"
        );
        assert_ne!(anonymous, configured);
    }

    /// 匿名请求 VIP 曲目时网易云回的是空 `url`（`br` 也是 0），走的是「无可用直链」
    /// 分支而不是 `freeTrialInfo` 分支；这条文案同样必须给出设置页路径，否则用户
    /// 看到「可能需要登录」也不知道该登录到哪里。
    #[test]
    fn empty_url_message_points_at_the_settings_page_without_an_account() {
        let anonymous = empty_url_message(false);
        assert!(anonymous.contains("网易云"), "{anonymous}");
        assert!(
            anonymous.contains("设置 → 平台账号") && anonymous.contains("Cookie"),
            "未配置账号的文案必须给出落地页：{anonymous}"
        );
        assert!(
            anonymous.contains("付费/VIP"),
            "匿名取不到时要点明可能的原因：{anonymous}"
        );
        for forbidden in ["其它音源", "换源", "回退", "替代音源"] {
            assert!(
                !anonymous.contains(forbidden),
                "拒绝文案不得跨源: {anonymous}"
            );
        }

        let configured = empty_url_message(true);
        assert_eq!(configured, ACCOUNT_NO_PERMISSION_MESSAGE);
        assert_ne!(anonymous, configured);
    }
}
