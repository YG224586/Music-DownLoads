//! 咪咕音乐搜索响应解析。
//!
//! 咪咕搜索接口返回的歌曲对象字段清单见 `_dev/probe/migu-probe-e7.log`
//! 的「第 1 条全部键」，其中**没有任何时长字段**（`duration` / `length` /
//! `songLength` / `dt` 全部不存在），因此契约 §2 的 `duration` 只能填 0。
//!
//! 音质方面：匿名请求下咪咕会忽略 `toneFlag`，实测（两首曲目 × 5 个档位，
//! `_dev/probe/migu-probe-e2.log`…`e5.log`）PQ/HQ/SQ/ZQ/LQ 拿到的都是同一个
//! `MP3_128_16_Stero/*.mp3`，所以匿名只声明 `128kmp3` 一档（宁缺勿假：声明 HQ
//! 会拿到「文件名说 HQ、字节其实是 128k」的假货）。**已配置账号**时追加
//! `320kmp3`（`HQ`）与 `flac`（`SQ`），由账号权益决定能否签发；拿不到时
//! `link.rs` 给「查权益/查有效期」的确定性中文文案。

use crate::platforms::{account_state, Platform};
use serde_json::{json, Value};

/// 对外声明的标准音质，取值来自冻结音质词表（契约 §5）。
pub const QUALITY: &str = "128kmp3";

/// 取链使用的咪咕档位名，对应标准音质。
pub const TONE_FLAG: &str = "PQ";

/// 标准音质的真实容器。
pub const EXTENSION: &str = "mp3";

/// 只有配置了账号才声明的两档：`(契约音质, 咪咕档位, 默认容器)`。
const ACCOUNT_TIERS: [(&str, &str, &str); 2] = [("320kmp3", "HQ", "mp3"), ("flac", "SQ", "flac")];

/// 取字符串字段；空字符串与缺失都视为「没有」。
fn text(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(text)) if !text.is_empty() => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

/// 把字符串或数字字段解析为 `u64`（咪咕的 `size`、`totalCount` 都是字符串）。
pub fn number(value: &Value) -> u64 {
    if let Some(number) = value.as_u64() {
        return number;
    }
    if let Some(number) = value.as_f64() {
        return number.max(0.0) as u64;
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

/// 解析歌手的 `{id, mid, name, coverUrl}` 数组；咪咕没有歌手头像，`coverUrl` 留空。
fn parse_artists(item: &Value) -> Vec<Value> {
    let Some(singers) = item.get("singers").and_then(Value::as_array) else {
        return Vec::new();
    };
    singers
        .iter()
        .filter_map(|singer| {
            let name = text(singer, "name")?;
            Some(json!({
                "id": text(singer, "id").unwrap_or_default(),
                "mid": "",
                "name": name,
                "coverUrl": ""
            }))
        })
        .collect()
}

/// 封面地址：优先 `imgItems` 里最小的 `01` 图，其次 `02`、`03`、任意一张。
fn cover_url(item: &Value) -> String {
    let Some(images) = item.get("imgItems").and_then(Value::as_array) else {
        return String::new();
    };
    for size_type in ["01", "02", "03"] {
        let found = images
            .iter()
            .filter(|image| text(image, "imgSizeType").as_deref() == Some(size_type))
            .find_map(|image| text(image, "img"));
        if let Some(url) = found {
            return url;
        }
    }
    images
        .iter()
        .find_map(|image| text(image, "img"))
        .unwrap_or_default()
}

/// 音质项的 `filename` 编码为 `{toneFlag}.{扩展名}`（如 `PQ.mp3`）。
///
/// 与酷我 `{bitrate}.{format}` 同思路：取链时从 filename 反解出请求档位，
/// 同时保证 `download/path.rs` 取到的落盘扩展名就是流的真实容器（契约 §3）。
pub fn quality_filename(tone_flag: &str, extension: &str) -> String {
    format!("{}.{}", tone_flag, extension)
}

/// 某个档位在这首歌里的声明信息：`(大小, 容器)`。
///
/// 优先 `newRateFormats`，其次 `rateFormats`；同一档位有多条记录时取第一条非零大小
/// 与非空容器。记录缺失时大小为 0、容器为 `None`（由调用方给默认容器）。
fn format_declaration(item: &Value, format_type: &str) -> (u64, Option<String>) {
    let mut size = 0;
    let mut extension = None;
    for key in ["newRateFormats", "rateFormats"] {
        let Some(formats) = item.get(key).and_then(Value::as_array) else {
            continue;
        };
        for format in formats {
            if text(format, "formatType").as_deref() != Some(format_type) {
                continue;
            }
            if size == 0 {
                let android = number(format.get("androidSize").unwrap_or(&Value::Null));
                size = if android > 0 {
                    android
                } else {
                    number(format.get("size").unwrap_or(&Value::Null))
                };
            }
            if extension.is_none() {
                extension = text(format, "androidFileType").or_else(|| text(format, "fileType"));
            }
        }
    }
    (size, extension)
}

/// 构造一个音质项：`filename` 编码该档位的 `{toneFlag}.{容器}`（见 [`quality_filename`]）。
fn quality_item(item: &Value, quality: &str, tone_flag: &str, default_extension: &str) -> Value {
    let (size, declared) = format_declaration(item, tone_flag);
    json!({
        "quality": quality,
        "size": size,
        "filename": quality_filename(tone_flag, declared.as_deref().unwrap_or(default_extension))
    })
}

/// 构造音质数组：匿名只有 `128kmp3` 一档（见模块文档），配置账号后追加 320k 与无损。
pub fn build_qualities(item: &Value) -> Vec<Value> {
    build_qualities_for(
        item,
        account_state::platform_account_configured(Platform::Migu),
    )
}

/// 档位构造的纯函数版本（账号态由参数给定），供单元测试与调用方显式指定。
pub fn build_qualities_for(item: &Value, account: bool) -> Vec<Value> {
    let mut qualities = vec![quality_item(item, QUALITY, TONE_FLAG, EXTENSION)];

    if account {
        for (quality, tone_flag, default_extension) in ACCOUNT_TIERS {
            qualities.push(quality_item(item, quality, tone_flag, default_extension));
        }
    }

    qualities
}

/// 把一首原始歌曲解析为契约 §2 规定的对象；缺少 `contentId` 时无法取链，返回 `None`。
pub fn parse_song(item: &Value, separator: &str) -> Option<Value> {
    let content_id = text(item, "contentId")?;
    let copyright_id = text(item, "copyrightId").unwrap_or_default();
    // 咪咕取链需要 copyrightId + contentId，契约允许 `mid` 使用复合编码。
    let mid = format!("{}|{}", copyright_id, content_id);

    let artists = parse_artists(item);
    let names: Vec<&str> = artists
        .iter()
        .filter_map(|artist| artist["name"].as_str())
        .collect();
    let album = item
        .get("albums")
        .and_then(Value::as_array)
        .and_then(|albums| albums.first());

    Some(json!({
        // contentId 是 18 位数字字符串：归一化成 u64 后仍精确，取链仍走上面的复合 mid。
        "id": crate::task::contract::song_id_to_u64(&content_id),
        "mid": mid.clone(),
        "title": text(item, "name").unwrap_or_default(),
        "artist": names.join(separator),
        "artists": artists,
        "album": album.and_then(|album| text(album, "name")).unwrap_or_default(),
        "albumId": album.and_then(|album| text(album, "id")).unwrap_or_default(),
        "albumMid": "",
        "duration": 0,
        "coverUrl": cover_url(item),
        "mediaMid": mid,
        "qualities": build_qualities(item)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_song() -> Value {
        serde_json::from_str(
            r#"{
                "id": "1125329711",
                "copyrightId": "60054704101",
                "contentId": "600913000007163534",
                "name": "晴天 (Live)",
                "singers": [
                    { "id": "112", "name": "周杰伦" },
                    { "id": "999", "name": "嘉宾" }
                ],
                "albums": [{ "id": "1125329686", "name": "周杰伦地表最强世界巡回演唱会" }],
                "imgItems": [
                    { "imgSizeType": "03", "img": "https://example.com/large.webp" },
                    { "imgSizeType": "01", "img": "https://example.com/small.webp" }
                ],
                "rateFormats": [
                    { "formatType": "PQ", "size": "3997991", "fileType": "mp3" },
                    { "formatType": "SQ", "androidSize": "30477128", "androidFileType": "flac" }
                ]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn song_follows_contract_shape() {
        let song = parse_song(&sample_song(), " / ").unwrap();
        assert_eq!(song["id"], 600913000007163534u64);
        assert_eq!(song["mid"], "60054704101|600913000007163534");
        assert_eq!(song["mediaMid"], song["mid"]);
        assert_eq!(song["title"], "晴天 (Live)");
        assert_eq!(song["artist"], "周杰伦 / 嘉宾");
        assert_eq!(song["artists"][0]["id"], "112");
        assert_eq!(song["artists"][0]["name"], "周杰伦");
        assert_eq!(song["album"], "周杰伦地表最强世界巡回演唱会");
        assert_eq!(song["albumId"], "1125329686");
        // 咪咕搜索响应没有任何时长字段，契约要求填 0。
        assert_eq!(song["duration"].as_u64(), Some(0));
        // 优先取 01（最小）封面。
        assert_eq!(song["coverUrl"], "https://example.com/small.webp");
    }

    #[test]
    fn qualities_only_declare_measured_standard_quality() {
        let song = parse_song(&sample_song(), " / ").unwrap();
        let qualities = song["qualities"].as_array().unwrap();
        assert_eq!(qualities.len(), 1);
        assert_eq!(qualities[0]["quality"], "128kmp3");
        assert_eq!(qualities[0]["filename"], "PQ.mp3");
        assert_eq!(qualities[0]["size"].as_u64(), Some(3997991));
    }

    #[test]
    fn size_falls_back_to_new_rate_formats_and_zero() {
        let item = json!({
            "contentId": "1",
            "newRateFormats": [{ "formatType": "PQ", "androidSize": "12345" }]
        });
        assert_eq!(build_qualities(&item)[0]["size"].as_u64(), Some(12345));
        assert_eq!(
            build_qualities(&json!({ "contentId": "1" }))[0]["size"].as_u64(),
            Some(0)
        );
    }

    #[test]
    fn configured_account_adds_high_and_lossless_tiers() {
        let item = sample_song();

        // 匿名：只有 PQ 一档（咪咕匿名忽略 toneFlag，声明更多就是假档位）。
        assert_eq!(build_qualities_for(&item, false).len(), 1);

        // 账号态：追加 HQ.mp3（320k）与 SQ.flac（无损，容器取记录里的 androidFileType）。
        let qualities = build_qualities_for(&item, true);
        assert_eq!(qualities.len(), 3);
        assert_eq!(qualities[1]["quality"], "320kmp3");
        assert_eq!(qualities[1]["filename"], "HQ.mp3");
        assert_eq!(qualities[2]["quality"], "flac");
        assert_eq!(qualities[2]["filename"], "SQ.flac");
        assert_eq!(qualities[2]["size"].as_u64(), Some(30477128));
    }

    #[test]
    fn song_without_content_id_is_skipped() {
        assert!(parse_song(&json!({ "name": "无内容 ID" }), " / ").is_none());
    }
}
