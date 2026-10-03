//! 网易云搜索结果解析。
//!
//! 搜索接口不返回封面地址（只有加密的 `picId`，需要网易云的 id 加密算法才能还原），
//! 也不返回各档位文件大小（要逐首再请求详情，会触发风控），因此 `coverUrl` 留空、
//! `size` 填 0，与契约「拿不到填 0 / 尽量非空」一致。
//!
//! 音质档位按「宁缺勿假」原则给出：匿名态实测只有 128k 与 320k mp3 能拿到**完整**
//! 曲目（`br=128000` → `lMusic` 大小，`br=320000` → `hMusic` 大小）；请求无损
//! （`br=999000`）时接口**静默降级**为 320k（返回 `br=320000`、`level=exhigh`），
//! 所以匿名不列出 `flac`。**已配置账号**时追加 `2000.flac`：登录态走
//! `enhance/player/url/v1` 的 `level=lossless`，账号有权益才有 flac，拿不到时
//! `link.rs` 给「查权益/查有效期」的确定性中文文案，不会把 320k 当无损交付。
//!
//! 档位**不再按 `fee` 分叉**：付费曲目（`fee=1`/`4`）匿名时也声明标准两档，
//! 这样用户点下载能拿到 `link.rs` 的可执行提示（需要在「设置 → 平台账号」填 Cookie），
//! 而不是空档位导致的「所选音质不可用」。

use serde_json::{json, Value};

use crate::platforms::{account_state, Platform};

/// 构造 `qualities` 数组：元素形状与酷我一致（`{quality, size, filename}`），
/// `filename` 为 `{bitrate}.{format}`，`link.rs` 从扩展名取容器、从主干取码率。
pub fn build_qualities() -> Vec<Value> {
    build_qualities_for(account_state::platform_account_configured(
        Platform::Netease,
    ))
}

/// 档位构造的纯函数版本（账号态由参数给定），供单元测试与调用方显式指定。
pub fn build_qualities_for(account: bool) -> Vec<Value> {
    let mut qualities = vec![
        json!({ "quality": "128kmp3", "size": 0, "filename": "128.mp3" }),
        json!({ "quality": "320kmp3", "size": 0, "filename": "320.mp3" }),
    ];

    if account {
        // 无损：只有登录态能拿到，匿名请求会被静默降级，所以仅在配置账号后声明。
        qualities.push(json!({ "quality": "flac", "size": 0, "filename": "2000.flac" }));
    }

    qualities
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
        "qualities": build_qualities(),
    })
}

#[cfg(test)]
mod tests {
    use super::{build_qualities_for, parse_song};
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
    fn paid_song_still_lists_standard_qualities() {
        // 匿名态下付费曲目也声明标准两档：点下载后由 link.rs 给出
        // 「需要网易云会员账号，请在设置 → 平台账号填入 Cookie」的可执行提示，
        // 而不是空档位导致的「所选音质不可用」。
        let song = json!({ "id": 1945894789u64, "name": "晴天 (钢琴版)", "fee": 1 });
        let parsed = parse_song(&song, "、");

        let qualities = parsed["qualities"].as_array().unwrap();
        assert_eq!(qualities.len(), 2);
        assert_eq!(qualities[0]["filename"], "128.mp3");
        assert_eq!(qualities[1]["filename"], "320.mp3");
    }

    #[test]
    fn configured_account_adds_the_lossless_tier() {
        // 匿名：不给 flac（请求无损会被静默降级成 320k）。
        let anonymous = build_qualities_for(false);
        assert_eq!(anonymous.len(), 2);

        // 账号态：追加 2000.flac，对应 link.rs 的 level=lossless。
        let configured = build_qualities_for(true);
        assert_eq!(configured.len(), 3);
        assert_eq!(configured[2]["quality"], "flac");
        assert_eq!(configured[2]["filename"], "2000.flac");
        assert_eq!(configured[2]["size"], 0);
    }
}
