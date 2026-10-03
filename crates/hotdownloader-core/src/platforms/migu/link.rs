//! 咪咕音乐取链模块。
//!
//! 调用 `listenSong.do`：对可播放曲目咪咕直接 **302** 到签名 CDN 直链
//! （在 `Location` 响应头里，host `freetyst.nf.migu.cn`），body 为空；
//! 对不可播放曲目返回 200 + `{"code":"200000","info":"…"}`。
//!
//! 实测要点（`_dev/probe/migu-probe-e2.log`…`e7.log`）：
//! - 匿名请求下 `toneFlag` 被忽略，任何档位拿到的都是 128kbps mp3；
//! - 签名 URL 可重复使用（同一 URL 连续两次 `Range: bytes=0-1` 都是 206），
//!   所以先用调用方 client 跟一次重定向确认真实容器，再交给下载层是安全的；
//! - reqwest 0.12 没有「按请求关闭重定向」的 API，只能沿用调用方 client 的策略，
//!   因此这里同时兼容「已跟随重定向拿到 200」与「拿到 302 + Location」两种情况。

use std::path::Path;

use reqwest::header::{CONTENT_TYPE, LOCATION};
use reqwest::{Client, Url};
use serde_json::Value;

use super::CHANNEL;

/// 取链接口地址（MIGUM3.0 同路径表现一致）。
const LISTEN_URL: &str = "https://app.c.nf.migu.cn/MIGUM2.0/v1.0/content/sub/listenSong.do";

/// 咪咕已知的档位名；当前只有 `PQ` 实测可用，其余档位仅作为 filename 的合法取值。
const KNOWN_TONES: [&str; 6] = ["LQ", "PQ", "HQ", "SQ", "ZQ", "Z3D"];

/// 咪咕支持的音频容器（契约 §3 要求扩展名即真实容器）。
const KNOWN_EXTENSIONS: [&str; 5] = ["mp3", "flac", "m4a", "ogg", "aac"];

/// 获取下载直链，返回 `(直链, 解密密钥)`。
///
/// 咪咕直链是明文音频（mp3），因此第二个返回值恒为空串。
///
/// # 参数
/// - `client`：调用方复用的 HTTP 客户端。
/// - `song_mid`：`{copyrightId}|{contentId}`，由 [`super::parser::parse_song`] 生成。
/// - `filename`：`{toneFlag}.{扩展名}`，由 [`super::parser::quality_filename`] 生成。
pub async fn get_download_link(
    client: &Client,
    song_mid: &str,
    filename: &str,
) -> Result<(String, String), String> {
    let (copyright_id, content_id) = parse_song_mid(song_mid)?;
    let (tone_flag, requested_extension) = parse_quality_filename(filename)?;
    let url = listen_url(&tone_flag, &copyright_id, &content_id)?;

    let response = super::app_request(client.get(url))
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let status = response.status();

    // 调用方 client 未跟随重定向时，直链就在 Location 里。
    if status.is_redirection() {
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| format!("咪咕未返回直链（HTTP {}）", status.as_u16()))?;
        return finalize(location, None, &requested_extension);
    }

    if !status.is_success() {
        return Err(format!("咪咕取链失败: HTTP {}", status.as_u16()));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    // 200 且是 JSON：咪咕用中文 info 说明为何不给直链（如会员曲目、已下线）。
    if content_type
        .as_deref()
        .is_some_and(|value| value.contains("json"))
    {
        let text = response
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {}", e))?;
        let data: Value =
            serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;
        let info = data
            .get("info")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(describe_error(info));
    }

    let direct_url = response.url().as_str().to_string();
    finalize(&direct_url, content_type.as_deref(), &requested_extension)
}

/// 校验直链可用且容器与请求音质一致，返回 `(直链, 解密密钥)`。
fn finalize(
    direct_url: &str,
    content_type: Option<&str>,
    requested_extension: &str,
) -> Result<(String, String), String> {
    if !direct_url.starts_with("http") {
        return Err(format!("咪咕返回的直链无效: {}", direct_url));
    }

    let actual = url_extension(direct_url).or_else(|| content_type_extension(content_type));
    if let Some(actual) = actual {
        if actual != requested_extension {
            return Err(format!(
                "咪咕返回的音频容器为 {}，与请求的 {} 不一致（该音质当前不可用）",
                actual, requested_extension
            ));
        }
    }

    // 咪咕直链是明文音频，没有加密容器，密钥恒为空。
    Ok((direct_url.to_string(), String::new()))
}

/// 从直链路径里取扩展名。
fn url_extension(url: &str) -> Option<String> {
    let path = Url::parse(url).ok()?.path().to_string();
    let extension = Path::new(&path).extension()?.to_str()?.to_ascii_lowercase();
    if extension.is_empty() {
        None
    } else {
        Some(extension)
    }
}

/// `Content-Type` → 容器扩展名。
fn content_type_extension(content_type: Option<&str>) -> Option<String> {
    let value = content_type?.split(';').next()?.trim().to_ascii_lowercase();
    match value.as_str() {
        "audio/mpeg" | "audio/mp3" => Some("mp3".to_string()),
        "audio/flac" | "audio/x-flac" => Some("flac".to_string()),
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" => Some("m4a".to_string()),
        "audio/ogg" | "application/ogg" => Some("ogg".to_string()),
        "audio/aac" | "audio/x-aac" => Some("aac".to_string()),
        _ => None,
    }
}

/// 把咪咕的中文 `info` 映射为对用户可读的失败文案。
///
/// 实测（`_dev/probe/migu-probe-e2.log`）：VIP 独占曲返回「暂不提供试听地址」，
/// 伪造/已下线曲目返回「歌曲下线暂不支持播放，敬请期待」。
fn describe_error(info: &str) -> String {
    match info {
        "暂不提供试听地址" => {
            "咪咕：该曲目暂不提供试听地址（可能需要咪咕会员）".to_string()
        }
        "歌曲下线暂不支持播放，敬请期待" => {
            "咪咕：该曲目已下线，暂不支持播放".to_string()
        }
        other => format!("咪咕取链失败: {}", other),
    }
}

/// 解析 `{copyrightId}|{contentId}`。
fn parse_song_mid(song_mid: &str) -> Result<(String, String), String> {
    let (copyright_id, content_id) = song_mid
        .split_once('|')
        .ok_or_else(|| format!("无效的咪咕歌曲标识: {}", song_mid))?;
    if copyright_id.is_empty() || content_id.is_empty() {
        return Err(format!("咪咕歌曲标识缺少版权或内容 ID: {}", song_mid));
    }
    Ok((copyright_id.to_string(), content_id.to_string()))
}

/// 解析 `{toneFlag}.{扩展名}`。
fn parse_quality_filename(filename: &str) -> Result<(String, String), String> {
    let (tone_flag, extension) = filename
        .split_once('.')
        .ok_or_else(|| format!("filename 缺少扩展名: {}", filename))?;
    let extension = extension.to_ascii_lowercase();
    if !KNOWN_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!("不支持的音频容器: {}", extension));
    }
    let tone_flag = tone_flag.to_ascii_uppercase();
    if !KNOWN_TONES.contains(&tone_flag.as_str()) {
        return Err(format!("不支持的咪咕音质档位: {}", tone_flag));
    }
    Ok((tone_flag, extension))
}

/// 构造取链 URL（参数与实测脚本 `_dev/probe/migu-probe.mjs` 完全一致）。
fn listen_url(tone_flag: &str, copyright_id: &str, content_id: &str) -> Result<Url, String> {
    let mut url = Url::parse(LISTEN_URL).map_err(|e| format!("URL 构建失败: {}", e))?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("toneFlag", tone_flag);
        params.append_pair("copyrightId", copyright_id);
        params.append_pair("contentId", content_id);
        params.append_pair("resourceType", "2");
        params.append_pair("channel", CHANNEL);
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_mid_round_trip() {
        let (copyright_id, content_id) = parse_song_mid("60054704101|600913000007163534").unwrap();
        assert_eq!(copyright_id, "60054704101");
        assert_eq!(content_id, "600913000007163534");
        assert!(parse_song_mid("600913000007163534").is_err());
        assert!(parse_song_mid("|600913000007163534").is_err());
        assert!(parse_song_mid("60054704101|").is_err());
    }

    #[test]
    fn quality_filename_encodes_tone_and_container() {
        assert_eq!(
            parse_quality_filename("PQ.mp3").unwrap(),
            ("PQ".to_string(), "mp3".to_string())
        );
        assert_eq!(
            parse_quality_filename("pq.MP3").unwrap(),
            ("PQ".to_string(), "mp3".to_string())
        );
        assert!(parse_quality_filename("PQ").is_err());
        assert!(parse_quality_filename("PQ.ape").is_err());
        assert!(parse_quality_filename("XX.mp3").is_err());
    }

    #[test]
    fn default_quality_filename_is_accepted() {
        let filename = super::super::parser::quality_filename(
            super::super::parser::TONE_FLAG,
            super::super::parser::EXTENSION,
        );
        assert_eq!(filename, "PQ.mp3");
        assert!(parse_quality_filename(&filename).is_ok());
    }

    #[test]
    fn mid_from_parser_is_accepted_by_link() {
        // search.rs 产出的 mid 与 link.rs 解析的 mid 必须是同一种编码，
        // 且两个 id 只含数字，「|」只作为分隔符出现（不会被 URL 二次拼接破坏）。
        let song = super::super::parser::parse_song(
            &serde_json::json!({
                "contentId": "600913000007163534",
                "copyrightId": "60054704101"
            }),
            " / ",
        )
        .unwrap();
        let mid = song["mid"].as_str().unwrap();
        assert_eq!(mid, "60054704101|600913000007163534");
        assert_eq!(mid.matches('|').count(), 1);
        let (copyright_id, content_id) = parse_song_mid(mid).unwrap();
        assert_eq!(copyright_id, "60054704101");
        assert_eq!(content_id, "600913000007163534");
        assert!(copyright_id.chars().all(|c| c.is_ascii_digit()));
        assert!(content_id.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn listen_url_uses_measured_parameters() {
        let url = listen_url("PQ", "60054704101", "600913000007163534").unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(url.path(), "/MIGUM2.0/v1.0/content/sub/listenSong.do");
        assert_eq!(params["toneFlag"], "PQ");
        assert_eq!(params["copyrightId"], "60054704101");
        assert_eq!(params["contentId"], "600913000007163534");
        assert_eq!(params["resourceType"], "2");
        assert_eq!(params["channel"], CHANNEL);
    }

    #[test]
    fn direct_link_container_must_match_requested_quality() {
        let url =
            "https://freetyst.nf.migu.cn/public/x/MP3_128_16_Stero/60054704101123747.mp3?Key=abc";
        assert!(finalize(url, Some("audio/mpeg"), "mp3").is_ok());
        let error = finalize(url, Some("audio/mpeg"), "flac").unwrap_err();
        assert!(error.contains("mp3") && error.contains("flac"));
        assert!(finalize("not-a-url", None, "mp3").is_err());
    }

    #[test]
    fn container_falls_back_to_content_type() {
        let url = "https://freetyst.nf.migu.cn/public/x/stream?Key=abc";
        assert!(finalize(url, Some("audio/mpeg"), "mp3").is_ok());
        assert!(finalize(url, Some("audio/flac"), "mp3").is_err());
    }

    #[test]
    fn known_denials_are_explained_in_chinese() {
        assert!(describe_error("暂不提供试听地址").contains("会员"));
        assert!(describe_error("歌曲下线暂不支持播放，敬请期待").contains("下线"));
        assert!(describe_error("别的错误").contains("别的错误"));
    }
}
