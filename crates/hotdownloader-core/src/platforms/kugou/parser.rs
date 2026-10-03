//! 酷狗搜索结果的字段归一化，以及 `mid` / 音质文件名编码。
//!
//! 契约（`_dev/platform-contract-v1.md`）：歌对象键固定为
//! `id/mid/title/artist/artists/album/albumId/albumMid/duration/coverUrl/mediaMid/qualities`，
//! `qualities` 每项为 `{quality,size,filename}`；`mid` 由本模块自定义编码，下载时原样回传。
//!
//! `mid` 编码为 `{128k hash}|{320k hash}|{无损 hash}|{专辑 id}`：酷狗每档音质对应不同
//! `hash`，而契约只允许一个 `mid` 字段承载，无法新增任务字段。

use std::path::Path;

use serde_json::{json, Value};

/// 复合 `mid` 的分段数。
const MID_SEGMENTS: usize = 4;

/// 封面模板里的尺寸占位符替换值。
const COVER_SIZE: &str = "480";

/// 酷狗音质档位。
///
/// 只保留**实测跑通**的三档（宁缺勿假）：探测里 `hires`/`全景声`/`蝰蛇母带` 在搜索响应中
/// 没有对应 `hash` 字段，无法取链，因此不写入 `qualities`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    /// standard → 128k mp3（`m.kugou.com` getSongInfo 接口，实测无需签名）。
    Standard,
    /// exhigh → 320k mp3（`trackercdn` v2，`br=hq`）。
    Exhigh,
    /// lossless → flac（`trackercdn` v2，`br=flac`）。
    Lossless,
}

impl Quality {
    /// 契约冻结的音质标签。
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "128kmp3",
            Self::Exhigh => "320kmp3",
            Self::Lossless => "flac",
        }
    }

    /// 直链真实容器，也是落盘扩展名（`download::path` 只取 `filename` 的扩展名）。
    pub fn extension(self) -> &'static str {
        match self {
            Self::Standard | Self::Exhigh => "mp3",
            Self::Lossless => "flac",
        }
    }

    /// 档位在 `filename` 中的编码值，沿用酷我 `{bitrate}.{format}` 约定
    /// （`128.mp3` / `320.mp3` / `2000.flac`）。
    pub fn bitrate(self) -> u32 {
        match self {
            Self::Standard => 128,
            Self::Exhigh => 320,
            Self::Lossless => 2000,
        }
    }

    /// 本地文件名。
    pub fn filename(self) -> String {
        format!("{}.{}", self.bitrate(), self.extension())
    }

    /// 从 `filename` 的 stem 解析档位；同时兼容契约标签形式（`128kmp3`/`320kmp3`/`flac`）。
    fn from_stem(stem: &str) -> Option<Self> {
        match stem {
            "128" | "128kmp3" => Some(Self::Standard),
            "320" | "320kmp3" => Some(Self::Exhigh),
            "2000" | "flac" => Some(Self::Lossless),
            _ => None,
        }
    }
}

/// 复合 `mid` 的解码结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KugouMid {
    /// 128k mp3 的 hash（必然存在）。
    pub hash_128: String,
    /// 320k mp3 的 hash，可能为空（该曲目没有 320k 资源）。
    pub hash_320: String,
    /// 无损 flac 的 hash，可能为空。
    pub hash_sq: String,
    /// 专辑 id，仅用于展示与去重。
    pub album_id: String,
}

impl KugouMid {
    /// 取指定档位的 hash。
    pub fn hash_for(&self, quality: Quality) -> &str {
        match quality {
            Quality::Standard => &self.hash_128,
            Quality::Exhigh => &self.hash_320,
            Quality::Lossless => &self.hash_sq,
        }
    }
}

/// 编码复合 `mid`。
pub fn encode_mid(hash_128: &str, hash_320: &str, hash_sq: &str, album_id: &str) -> String {
    format!("{}|{}|{}|{}", hash_128, hash_320, hash_sq, album_id)
}

/// 解析复合 `mid`：`{128k hash}|{320k hash}|{无损 hash}|{专辑 id}`。
///
/// 也接受裸 32 位 hex hash（此时只有 128k 档可用），便于历史任务记录或手工构造。
pub fn parse_mid(mid: &str) -> Result<KugouMid, String> {
    let parts: Vec<&str> = mid.split('|').collect();
    if parts.len() == MID_SEGMENTS {
        let hash_128 = parts[0].trim();
        if hash_128.is_empty() {
            return Err(format!("无效的酷狗歌曲标识: {}", mid));
        }
        return Ok(KugouMid {
            hash_128: hash_128.to_string(),
            hash_320: parts[1].trim().to_string(),
            hash_sq: parts[2].trim().to_string(),
            album_id: parts[3].trim().to_string(),
        });
    }

    let single = mid.trim();
    if is_hash(single) {
        return Ok(KugouMid {
            hash_128: single.to_ascii_lowercase(),
            hash_320: String::new(),
            hash_sq: String::new(),
            album_id: String::new(),
        });
    }

    Err(format!("无效的酷狗歌曲标识: {}", mid))
}

/// 解析任务文件名里的音质档位，并校验扩展名与档位一致
/// （落盘扩展名取自 `filename`，不一致会让扩展名说谎）。
pub fn parse_quality_filename(filename: &str) -> Result<Quality, String> {
    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let quality =
        Quality::from_stem(stem).ok_or_else(|| format!("不支持的酷狗音质: {}", filename))?;
    if extension != quality.extension() {
        return Err(format!(
            "文件名扩展名与音质不匹配（{} 应为 .{}）: {}",
            quality.label(),
            quality.extension(),
            filename
        ));
    }
    Ok(quality)
}

/// 把一条搜索记录归一化为契约歌对象。
///
/// 缺少必要字段（`hash`/`songname`/数字 id）时返回 `None`——这类条目无法搜索后下载
/// （落盘文件名需要标题，取链需要 hash）。本函数**不发起任何网络请求**。
pub fn parse_song(song: &Value, artist_separator: &str) -> Option<Value> {
    let hash_128 = text(song, "hash")?;
    let title = text(song, "songname")?;
    let id = number(song, "audio_id")
        .filter(|value| *value > 0)
        .or_else(|| number(song, "album_audio_id").filter(|value| *value > 0))?;

    let hash_320 = text(song, "320hash").unwrap_or_default();
    let hash_sq = text(song, "sqhash").unwrap_or_default();
    let album_id = id_text(song, "album_id");
    let artist = text(song, "singername").unwrap_or_default();
    let mid = encode_mid(&hash_128, &hash_320, &hash_sq, &album_id);

    Some(json!({
        "id": id,
        "mid": mid,
        "title": title,
        "artist": artist.as_str(),
        "artists": build_artists(&artist, artist_separator),
        "album": text(song, "album_name").unwrap_or_default(),
        "albumId": &album_id,
        "albumMid": &album_id,
        "duration": number(song, "duration").unwrap_or(0),
        "coverUrl": cover_url(song),
        "mediaMid": hash_128.as_str(),
        "qualities": build_qualities(song),
    }))
}

/// 只输出实测跑通的档位：对应 hash 为空（或缺字段）的档位不出现。
pub fn build_qualities(song: &Value) -> Vec<Value> {
    const TIERS: [(Quality, &str, &str); 3] = [
        (Quality::Standard, "hash", "filesize"),
        (Quality::Exhigh, "320hash", "320filesize"),
        (Quality::Lossless, "sqhash", "sqfilesize"),
    ];

    let mut qualities = Vec::new();
    for (quality, hash_key, size_key) in TIERS {
        if text(song, hash_key).is_none() {
            continue;
        }
        qualities.push(json!({
            "quality": quality.label(),
            "size": number(song, size_key).unwrap_or(0),
            "filename": quality.filename(),
        }));
    }
    qualities
}

/// 酷狗搜索只返回合并后的歌手名（如 `周杰伦`、`A、B`），按设置里的分隔符尽量拆分；
/// 拆不出名字时保留原始字符串。
fn build_artists(artist: &str, separator: &str) -> Vec<Value> {
    if artist.is_empty() {
        return Vec::new();
    }

    let names: Vec<&str> = if separator.is_empty() {
        vec![artist]
    } else {
        artist
            .split(separator)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect()
    };
    if names.is_empty() {
        return vec![artist_entry(artist)];
    }
    names.into_iter().map(artist_entry).collect()
}

/// 酷狗搜索响应不含歌手 id 与头像，用占位值满足契约键表。
fn artist_entry(name: &str) -> Value {
    json!({ "id": 0, "mid": "", "name": name, "coverUrl": "" })
}

/// 封面：酷狗给的是模板 `http://imge.kugou.com/stdmusic/{size}/….jpg`。
fn cover_url(song: &Value) -> String {
    let template = song
        .get("trans_param")
        .and_then(|param| param.get("union_cover"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    template.replace("{size}", COVER_SIZE)
}

/// 取非空字符串字段。
fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// 取无符号整数字段。
fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

/// 取字段的文本形式（酷狗的 id 类字段数字与字符串两种形态都出现过）。
fn id_text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    }
}

/// 32 位 hex。
fn is_hash(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{
        build_qualities, encode_mid, parse_mid, parse_quality_filename, parse_song, Quality,
    };
    use serde_json::Value;

    /// 实测样本（`_dev/probe/kugou-probe.log` 的 S1 命中：蓝心羽 - 晴天，免费曲目）。
    const SAMPLE: &str = r#"{
        "hash": "48c685f679ffc7cf08b8a8341ca9db44",
        "songname": "晴天",
        "singername": "蓝心羽",
        "album_name": "晴天",
        "album_id": 79360569,
        "duration": 176,
        "filesize": 2829338,
        "320hash": "28f83bbd8cab043895039e98366ad2b4",
        "320filesize": 7074003,
        "sqhash": "857879479e698d729640a5716dc3e56f",
        "sqfilesize": 14324496,
        "audio_id": 50359489,
        "album_audio_id": 130783548,
        "privilege": 0,
        "pay_type": 0,
        "trans_param": { "union_cover": "http://imge.kugou.com/stdmusic/{size}/20230920/20230920142503632013.jpg" }
    }"#;

    #[test]
    fn parse_song_normalizes_contract_keys() {
        let song: Value = serde_json::from_str(SAMPLE).unwrap();
        let parsed = parse_song(&song, "、").expect("样本应能解析");

        assert_eq!(parsed["id"], 50359489u64);
        assert_eq!(
            parsed["mid"],
            "48c685f679ffc7cf08b8a8341ca9db44|28f83bbd8cab043895039e98366ad2b4|857879479e698d729640a5716dc3e56f|79360569"
        );
        assert_eq!(parsed["title"], "晴天");
        assert_eq!(parsed["artist"], "蓝心羽");
        assert_eq!(parsed["album"], "晴天");
        assert_eq!(parsed["albumId"], "79360569");
        assert_eq!(parsed["albumMid"], "79360569");
        assert_eq!(parsed["duration"], 176u64);
        assert_eq!(parsed["mediaMid"], "48c685f679ffc7cf08b8a8341ca9db44");
        assert_eq!(
            parsed["coverUrl"],
            "http://imge.kugou.com/stdmusic/480/20230920/20230920142503632013.jpg"
        );
        assert_eq!(parsed["artists"][0]["name"], "蓝心羽");
        assert_eq!(parsed["artists"][0]["id"].as_u64(), Some(0));
        assert_eq!(parsed["qualities"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn build_qualities_only_lists_probed_tiers() {
        let song: Value = serde_json::from_str(SAMPLE).unwrap();
        let qualities = build_qualities(&song);
        assert_eq!(qualities.len(), 3);
        assert_eq!(qualities[0]["quality"], "128kmp3");
        assert_eq!(qualities[0]["size"], 2829338u64);
        assert_eq!(qualities[0]["filename"], "128.mp3");
        assert_eq!(qualities[1]["quality"], "320kmp3");
        assert_eq!(qualities[1]["size"], 7074003u64);
        assert_eq!(qualities[1]["filename"], "320.mp3");
        assert_eq!(qualities[2]["quality"], "flac");
        assert_eq!(qualities[2]["size"], 14324496u64);
        assert_eq!(qualities[2]["filename"], "2000.flac");

        // 只有 128k hash 的条目只出现一档；无损 hash 为空时不出现 flac。
        let only_128: Value = serde_json::json!({ "hash": "48c685f679ffc7cf08b8a8341ca9db44", "songname": "x", "audio_id": 1 });
        let limited = build_qualities(&only_128);
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0]["quality"], "128kmp3");
    }

    #[test]
    fn parse_song_skips_unusable_items() {
        assert!(parse_song(&serde_json::json!({}), "、").is_none());
        // 缺 hash
        assert!(parse_song(&serde_json::json!({ "songname": "x", "audio_id": 1 }), "、").is_none());
        // hash 为空串
        assert!(parse_song(
            &serde_json::json!({ "hash": "", "songname": "x", "audio_id": 1 }),
            "、"
        )
        .is_none());
        // 缺标题
        assert!(parse_song(
            &serde_json::json!({ "hash": "48c685f679ffc7cf08b8a8341ca9db44", "audio_id": 1 }),
            "、"
        )
        .is_none());
        // 缺数字 id（audio_id 为 0 时回退 album_audio_id）
        let fallback: Value = serde_json::json!({
            "hash": "48c685f679ffc7cf08b8a8341ca9db44",
            "songname": "x",
            "audio_id": 0,
            "album_audio_id": 130783548
        });
        assert_eq!(parse_song(&fallback, "、").unwrap()["id"], 130783548u64);
        assert!(parse_song(
            &serde_json::json!({ "hash": "48c685f679ffc7cf08b8a8341ca9db44", "songname": "x" }),
            "、"
        )
        .is_none());
    }

    #[test]
    fn artists_are_split_by_configured_separator() {
        let song: Value = serde_json::json!({
            "hash": "48c685f679ffc7cf08b8a8341ca9db44",
            "songname": "合唱",
            "singername": "A、B",
            "audio_id": 1
        });
        let parsed = parse_song(&song, "、").unwrap();
        assert_eq!(parsed["artists"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["artists"][1]["name"], "B");
        // 分隔符为空时保留整体
        let parsed = parse_song(&song, "").unwrap();
        assert_eq!(parsed["artists"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["artists"][0]["name"], "A、B");
    }

    #[test]
    fn mid_round_trips_and_rejects_garbage() {
        let mid = encode_mid(
            "a".repeat(32).as_str(),
            "b".repeat(32).as_str(),
            "",
            "79360569",
        );
        let parsed = parse_mid(&mid).unwrap();
        assert_eq!(parsed.hash_128, "a".repeat(32));
        assert_eq!(parsed.hash_320, "b".repeat(32));
        assert!(parsed.hash_sq.is_empty());
        assert_eq!(parsed.album_id, "79360569");
        assert_eq!(parsed.hash_for(Quality::Lossless), "");

        // 裸 hash 也接受（只有 128k 档）
        let bare = parse_mid("48C685F679FFC7CF08B8A8341CA9DB44").unwrap();
        assert_eq!(bare.hash_128, "48c685f679ffc7cf08b8a8341ca9db44");
        assert!(bare.hash_320.is_empty());

        assert!(parse_mid("").is_err());
        assert!(parse_mid("not-a-hash").is_err());
        assert!(parse_mid("|b|").is_err());
        assert!(parse_mid("48c685f679ffc7cf08b8a8341ca9db44|b").is_err());
    }

    #[test]
    fn quality_filename_matches_task_contract() {
        assert_eq!(
            parse_quality_filename("128.mp3").unwrap(),
            Quality::Standard
        );
        assert_eq!(parse_quality_filename("320.mp3").unwrap(), Quality::Exhigh);
        assert_eq!(
            parse_quality_filename("2000.flac").unwrap(),
            Quality::Lossless
        );
        // 兼容契约标签形式
        assert_eq!(
            parse_quality_filename("320kmp3.mp3").unwrap(),
            Quality::Exhigh
        );
        assert_eq!(
            parse_quality_filename("flac.flac").unwrap(),
            Quality::Lossless
        );
        // 扩展名与档位不一致必须报错（否则落盘扩展名会说谎）
        assert!(parse_quality_filename("320.flac").is_err());
        assert!(parse_quality_filename("2000.mp3").is_err());
        assert!(parse_quality_filename("hires.mp3").is_err());
        assert!(parse_quality_filename("mp3").is_err());
    }

    #[test]
    fn quality_filenames_are_unique_and_stable() {
        assert_eq!(Quality::Standard.filename(), "128.mp3");
        assert_eq!(Quality::Exhigh.filename(), "320.mp3");
        assert_eq!(Quality::Lossless.filename(), "2000.flac");
        assert_eq!(Quality::Standard.label(), "128kmp3");
        assert_eq!(Quality::Exhigh.label(), "320kmp3");
        assert_eq!(Quality::Lossless.label(), "flac");
        assert_eq!(Quality::Standard.extension(), "mp3");
        assert_eq!(Quality::Lossless.extension(), "flac");
    }
}
