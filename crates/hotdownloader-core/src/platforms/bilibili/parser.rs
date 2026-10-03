use serde_json::Value;

/// 解析 `search/type?search_type=video` 的单条视频，映射为契约歌曲对象
/// （键名与 `platforms/kuwo/parser.rs:220-233` 同构）。
pub fn parse_song(v: &Value, separator: &str) -> serde_json::Value {
    use serde_json::json;

    let bvid = v["bvid"].as_str().unwrap_or_default().to_string();
    let author = v["author"].as_str().unwrap_or_default().to_string();
    let title = clean_title(v["title"].as_str().unwrap_or_default());
    let duration = parse_duration(v["duration"].as_str().unwrap_or_default());
    // pic 是协议相对 URL（//i0.hdslb.com/...），前端直接用会因协议不明而加载失败。
    let cover = v["pic"].as_str().unwrap_or_default().to_string();
    let cover_url = if let Some(rest) = cover.strip_prefix("//") {
        format!("https://{}", rest)
    } else {
        cover
    };
    let _ = separator; // B 站结果单 UP 主，无多歌手拼接场景
    json!({
        // BV 号不是数字：归一化成稳定哈希，取链仍用 mid（= bvid）。
        "id": crate::task::contract::song_id_to_u64(&bvid),
        "mid": bvid,
        "title": title,
        "artist": author,
        "artists": [{
            "id": v["mid"].as_i64().unwrap_or_default().to_string(),
            "mid": v["mid"].as_i64().unwrap_or_default().to_string(),
            "name": author,
            "coverUrl": "",
        }],
        "album": "",
        "albumId": "",
        "albumMid": "",
        "duration": duration,
        "coverUrl": cover_url,
        "mediaMid": bvid,
        "qualities": [{
            "quality": "192kaac",
            "size": 0,
            "filename": "192kaac.m4a",
        }],
    })
}

/// 剥除标题中的 `<em class="keyword">` 高亮标签与常见 HTML 实体。
pub fn clean_title(raw: &str) -> String {
    let mut s = String::with_capacity(raw.len());
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => s.push(c),
            _ => {}
        }
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// "4:30" / "1:02:03" → 秒。
pub fn parse_duration(raw: &str) -> u64 {
    let mut total: u64 = 0;
    for part in raw.split(':') {
        match part.parse::<u64>() {
            Ok(n) => total = total * 60 + n,
            Err(_) => return 0,
        }
    }
    total
}
