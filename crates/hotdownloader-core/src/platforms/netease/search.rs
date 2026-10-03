//! 网易云音乐搜索：使用明文 web 接口，不需要 eapi/weapi 加密。

use std::time::Duration;

use serde_json::{json, Value};

use super::parser::parse_song;

/// 搜索结果条数上限，与前端每页请求量对齐。
const SEARCH_URL: &str = "https://music.163.com/api/search/get/web";

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const COOKIE: &str = "appver=8.7.01; os=pc";

/// 搜索歌曲。`page` 从 1 开始，返回 `{"songs": [...], "has_more": bool}`。
pub async fn search_songs(
    separator: &str,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = search_request(&keyword, page, limit).await?;

    let result = data.get("result").cloned().unwrap_or(Value::Null);
    let songs = result
        .get("songs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let parsed: Vec<Value> = songs
        .iter()
        .map(|song| parse_song(song, separator))
        .collect();

    let total = result.get("songCount").and_then(Value::as_u64).unwrap_or(0);

    let offset = u64::from(page.saturating_sub(1)) * u64::from(limit);
    let has_more = if total > 0 {
        offset + (parsed.len() as u64) < total
    } else {
        parsed.len() as u64 == u64::from(limit)
    };

    Ok(json!({ "songs": parsed, "has_more": has_more }).to_string())
}

async fn search_request(keyword: &str, page: u32, limit: u32) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("网络错误: 网易云搜索客户端创建失败: {error}"))?;

    let offset = u64::from(page.saturating_sub(1)) * u64::from(limit);

    let response = client
        .get(SEARCH_URL)
        .query(&[
            ("s", keyword.to_string()),
            ("type", "1".to_string()),
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ])
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::REFERER, "https://music.163.com/search/")
        .header(reqwest::header::COOKIE, COOKIE)
        .send()
        .await
        .map_err(|error| format!("网络错误: 网易云搜索请求失败: {error}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取响应失败: 网易云搜索响应读取失败: {error}"))?;

    let parsed: Value = serde_json::from_str(&text)
        .map_err(|error| format!("解析响应失败: 网易云搜索响应解析失败: {error}"))?;

    if !status.is_success() {
        return Err(format!("网易云搜索接口返回 HTTP {status}"));
    }

    Ok(parsed)
}
