//! 哔哩哔哩搜索：老版（非 wbi）视频区搜索接口。

use serde_json::{json, Value};

use super::{fetch_buvid_cookie, http_get_json};

/// 搜索视频区的音视频投稿，映射为契约歌曲对象（`artist` = UP 主名）。
///
/// 每页固定 20 条（接口不接受 page_size），`limit` 只做本地截断。
pub async fn search_songs(
    separator: &str,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let page = page.max(1);
    let url = format!(
        "https://api.bilibili.com/x/web-interface/search/type?search_type=video&keyword={}&page={}",
        urlencode(&keyword),
        page
    );
    let cookie = fetch_buvid_cookie().await?;
    let root = http_get_json(&url, &cookie).await?;

    if root["code"].as_i64() != Some(0) {
        return Err(format!(
            "搜索失败: {}",
            root["message"].as_str().unwrap_or("未知错误")
        ));
    }
    let result = match root["data"]["result"].as_array() {
        Some(a) => a,
        None => return Err("解析响应失败: data.result 不是数组".to_string()),
    };
    let num_pages = root["data"]["numPages"].as_i64().unwrap_or(0);
    let effective_limit = limit.min(20) as usize;
    let songs: Vec<Value> = result
        .iter()
        .take(effective_limit)
        .map(|v| super::parser::parse_song(v, separator))
        .collect();
    // has_more：接口返回总页数时用页数判断，否则退化为本页满页。
    let has_more = if num_pages > 0 {
        i64::from(page) < num_pages
    } else {
        songs.len() == effective_limit
    };
    Ok(json!({ "songs": songs, "has_more": has_more }).to_string())
}

/// `application/x-www-form-urlencoded` 风格的百分号编码（搜索关键词用）。
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencodes_utf8_keyword() {
        assert_eq!(urlencode("晴天"), "%E6%99%B4%E5%A4%A9");
        assert_eq!(urlencode("Jay & A~B"), "Jay%20%26%20A~B");
    }
}
