//! 网易云搜索结果解析。
//!
//! 搜索接口不返回封面地址（只有加密的 `picId`，需要网易云的 id 加密算法才能还原），
//! 也不返回各档位文件大小（要逐首再请求详情，会触发风控），因此 `coverUrl` 留空、
//! `size` 填 0，与契约「拿不到填 0 / 尽量非空」一致。
//!
//! 音质档位按「宁缺勿假」原则给出：匿名态实测只有 128k 与 320k mp3 能拿到**完整**
//! 曲目（`br=128000` → `lMusic` 大小，`br=320000` → `hMusic` 大小）；请求无损
//! （`br=999000`）时接口**静默降级**为 320k（返回 `br=320000`、`level=exhigh`），
//! 所以不列出 `flac`/`hires`。付费曲目（`fee=1`/`4`）匿名只给试听片段，不列出任何档位，
//! 由 `link.rs` 返回确定性的中文错误。

use serde_json::{json, Value};

/// 网易云 `fee` 语义：`0` 免费、`8` 低音质免费（标准音质可免费听）；
/// `1` 会员曲目、`4` 数字专辑，匿名只能试听。
fn allows_anonymous_download(fee: Option<i64>) -> bool {
    match fee {
        Some(value) => matches!(value, 0 | 8),
        // 字段缺失时按可下载处理，真正的可用性由 link.rs 校验直链兜底。
        None => true,
    }
}

/// 构造 `qualities` 数组：元素形状与酷我一致（`{quality, size, filename}`），
/// `filename` 为 `{bitrate}.{format}`，`link.rs` 从扩展名取容器、从主干取码率。
pub fn build_qualities(free: bool) -> Vec<Value> {
    if !free {
        return Vec::new();
    }

    vec![
        json!({ "quality": "128kmp3", "size": 0, "filename": "128.mp3" }),
        json!({ "quality": "320kmp3", "size": 0, "filename": "320.mp3" }),
    ]
}

fn entity_id(value: Option<&Value>) -> String {
    match value {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

fn text_of(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn parse_artists(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let id = entity_id(item.get("id"));
                    json!({
                        "id": id.clone(),
                        "mid": id,
                        "name": text_of(item.get("name")),
                        "coverUrl": "",
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 把一条搜索记录转成与酷我等平台同构的歌曲 JSON（键名逐字一致）。
pub fn parse_song(song: &Value, separator: &str) -> Value {
    let mid = entity_id(song.get("id"));
    let album = song.get("album");

    let artist = song
        .get("artists")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(separator)
        })
        .unwrap_or_default();

    // 搜索接口的 `duration` 是毫秒，契约要求秒。
    let duration = song
        .get("duration")
        .and_then(Value::as_u64)
        .map(|millis| millis / 1000)
        .unwrap_or(0);

    let free = allows_anonymous_download(song.get("fee").and_then(Value::as_i64));

    json!({
        // 网易云搜索返回的 id 是数字字符串，归一化成任务契约定义的 u64。
        "id": crate::task::contract::song_id_to_u64(&mid),
        "mid": mid.clone(),
        "title": text_of(song.get("name")),
        "artist": artist,
        "artists": parse_artists(song.get("artists")),
        "album": text_of(album.and_then(|value| value.get("name"))),
        "albumId": entity_id(album.and_then(|value| value.get("id"))),
        "albumMid": "",
        "duration": duration,
        "coverUrl": "",
        "mediaMid": mid,
        "qualities": build_qualities(free),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_song;
    use serde_json::json;

    #[test]
    fn song_keys_follow_shared_contract() {
        let song = json!({
            "id": 2652820720u64,
            "name": "晴天(深情版)",
            "duration": 278961,
            "fee": 8,
            "artists": [{ "id": 96154669u64, "name": "Lucky小爱" }],
            "album": { "id": 255723258u64, "name": "晴天(深情版)" },
        });

        let parsed = parse_song(&song, "、");

        assert_eq!(parsed["mid"], "2652820720");
        // 搜索返回的 id 是数字字符串，契约要求归一化成数字。
        assert_eq!(parsed["id"], 2652820720u64);
        assert_eq!(parsed["title"], "晴天(深情版)");
        assert_eq!(parsed["artist"], "Lucky小爱");
        assert_eq!(parsed["artists"][0]["mid"], "96154669");
        assert_eq!(parsed["album"], "晴天(深情版)");
        assert_eq!(parsed["albumId"], "255723258");
        assert_eq!(parsed["duration"], 278);
        assert_eq!(parsed["coverUrl"], "");
        assert_eq!(parsed["mediaMid"], "2652820720");
        assert_eq!(parsed["qualities"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn paid_song_lists_no_quality() {
        let song = json!({ "id": 1945894789u64, "name": "晴天 (钢琴版)", "fee": 1 });
        let parsed = parse_song(&song, "、");

        assert_eq!(parsed["qualities"].as_array().map(Vec::len), Some(0));
    }
}
