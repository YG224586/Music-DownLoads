//! 网易云取直链：明文 `enhance/player/url` 接口。
//!
//! 匿名（未登录）能力边界由 `_dev/probe/netease-probe.mjs` 实测确定：
//! - `br=128000` → `level=standard`，实际字节数与详情 `lMusic.size` 一致，完整曲目；
//! - `br=320000` → `level=exhigh`，实际字节数与详情 `hMusic.size` 一致，完整曲目；
//! - `br=999000`（无损）→ 静默降级为 `br=320000`，所以不提供 flac/hires；
//! - 付费曲目（`fee=1`/`4`）只返回试听片段（实测 720813 字节，详情 `hMusic` 为 9526125），
//!   由 `freeTrialInfo` 判定并确定性失败，绝不把片段当完整歌曲交付。

use reqwest::Client;
use serde_json::Value;

const PLAYER_URL: &str = "https://music.163.com/api/song/enhance/player/url";

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const COOKIE: &str = "appver=8.7.01; os=pc";

/// 取得 `(直链, 解密密钥)`。网易云返回的是明文 mp3，密钥恒为空串。
pub async fn get_download_link(
    client: &Client,
    song_mid: &str,
    filename: &str,
) -> Result<(String, String), String> {
    let song_id = parse_song_id(song_mid)?;
    let (bitrate, format) = parse_quality_filename(filename)?;

    if format != "mp3" {
        return Err(format!(
            "网易云匿名访问仅提供 mp3 音质，{format} 需要登录会员后才能获取"
        ));
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
        return Err("网易云未返回可用的音频直链（该曲目可能无版权、已下架或需要登录）".to_string());
    }

    if !entry
        .get("freeTrialInfo")
        .map(Value::is_null)
        .unwrap_or(true)
    {
        return Err("该曲目为付费曲目，未登录网易云仅提供试听片段，无法下载完整歌曲".to_string());
    }

    let actual_br = entry.get("br").and_then(Value::as_u64).unwrap_or(0);
    if actual_br != u64::from(requested_br) {
        return Err(format!(
            "网易云未提供所选音质（请求 {requested_br}，实际返回 {actual_br}），未登录仅能获取标准音质"
        ));
    }

    Ok((url, String::new()))
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

/// 解析网易云歌曲 ID；内置多音源回退（`download/fallback.rs`）也用它反查歌曲详情。
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
    use super::{parse_quality_filename, parse_song_id};

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
}
