//! 内置多音源回退：主平台取链失败时，用歌曲元信息在其它内置音源里重新解析直链。
//!
//! # 为什么需要反查歌曲信息
//!
//! `DownloadLinkProvider::fetch` 只拿得到 `platform` / `song_mid` / `filename` 三个参数
//! （trait 签名被冻结：`app/src-tauri/src/adapters/tauri_download_host.rs` 是纯委托实现，
//! 改动会波及该文件），所以回退必须自己反查标题、歌手与时长。
//! QQ 音乐的匿名歌曲详情接口正好一次给足这三项，用来在酷我搜索结果里做严格匹配。
//!
//! # 容器（扩展名）约束
//!
//! 落盘文件名在**取链之前**就由 [`crate::download::path::resolve_download_path`] 定好了，
//! 它只从任务文件名里取扩展名。因此回退返回的文件，其真实容器必须与任务原扩展名同类，
//! 否则会出现「扩展名骗人」的坏文件。跨容器的降级（例如要 flac 却只能给 mp3）
//! 宁可让任务失败，也不改名落地。
//!
//! # 反错歌
//!
//! 候选曲目必须同时满足：标题归一化后完全相等、歌手有交集、时长差 ≤ 3 秒。
//! 三项缺一即放弃回退（宁可失败也不能下成另一首歌）。

use reqwest::Client;
use serde_json::{json, Value};

use crate::platforms::kuwo::{link as kuwo_link, parser as kuwo_parser, search as kuwo_search};
use crate::platforms::qqmusic::search::pc_comm;
use crate::platforms::Platform;

/// 确定性的「找不到可回退音源」文案，直接展示给用户。
pub(crate) const NO_MATCH_MESSAGE: &str = "未在内置音源找到匹配的歌曲：QQ 音乐需要登录或该曲目受限";

/// QQ 歌曲详情接口（匿名可用），用于反查标题/歌手/时长。
const QQ_DETAIL_ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const QQ_DETAIL_MODULE: &str = "music.pf_song_detail_svr";
const QQ_DETAIL_METHOD: &str = "get_song_detail_yqq";

/// 酷我搜索一次拉取的候选数量。
const KUWO_SEARCH_LIMIT: u32 = 20;

/// 时长允许的最大偏差（秒）。超过该值认为是不同版本（Live/伴奏/翻录）。
const MAX_DURATION_DELTA: u64 = 3;

/// 回退成功后返回的直链信息。
pub(crate) struct FallbackLink {
    pub(crate) url: String,
    pub(crate) key: String,
    /// 实际提供直链的音源。
    pub(crate) source: Platform,
    /// 实际拿到的音质标签（与前端品质列表同一套文案）。
    pub(crate) quality: String,
}

/// 回退结论。调用方据此决定给用户展示哪条错误。
pub(crate) enum FallbackOutcome {
    /// 解析到可用直链。
    Linked(FallbackLink),
    /// 确定性失败：已尽力搜索，但内置音源给不出可信的匹配曲目。
    Unavailable(String),
    /// 临时性失败：网络/解析问题，调用方应保留主平台的原始错误（重试语义不变）。
    Transient(String),
}

/// 音源中文名，仅用于日志。
pub(crate) fn platform_label(platform: Platform) -> &'static str {
    match platform {
        Platform::QqMusic => "QQ 音乐",
        Platform::Kuwo => "酷我",
        Platform::Script(_) => "自定义音源",
    }
}

/// 主平台取链失败后的内置回退入口。
///
/// 当前内置的匿名可用音源只有酷我（实测酷狗下载需付费、咪咕直链需签名、
/// 网易云搜索结果错歌率高），因此只实现「QQ 音乐 → 酷我」这一个方向。
pub(crate) async fn fetch_from_other_sources(
    client: &Client,
    platform: Platform,
    song_mid: &str,
    filename: &str,
) -> FallbackOutcome {
    match platform {
        Platform::QqMusic => fallback_to_kuwo(client, song_mid, filename).await,
        // 酷我自身没有可替代的内置音源，保留主平台错误。
        Platform::Kuwo => FallbackOutcome::Unavailable("酷我任务没有可用的回退音源".to_string()),
        // 自定义音源脚本本身就是用户指定的来源，内置回退不接手。
        Platform::Script(_) => {
            FallbackOutcome::Unavailable("自定义音源任务不做内置回退".to_string())
        }
    }
}

/// QQ 音乐 → 酷我音乐的回退链路。
async fn fallback_to_kuwo(client: &Client, song_mid: &str, filename: &str) -> FallbackOutcome {
    let formats = match compatible_formats(filename) {
        Some(formats) => formats,
        None => {
            return FallbackOutcome::Unavailable(format!(
                "任务文件名 {} 的扩展名在酷我没有同类明文容器",
                filename
            ))
        }
    };
    let cap = requested_bitrate_cap(filename);

    // 1. 反查歌曲信息（标题/歌手/时长）
    let meta = match fetch_qq_song_meta(client, song_mid).await {
        Ok(meta) => meta,
        Err(error) => return classify(error),
    };
    if meta.title.is_empty() || meta.artist.is_empty() {
        return FallbackOutcome::Unavailable(format!(
            "QQ 音乐歌曲详情缺少标题或歌手（{}）",
            song_mid
        ));
    }

    // 2. 用「标题 歌手」在酷我搜索
    let keyword = format!("{} {}", meta.title, meta.artist);
    let data = match kuwo_search::search_raw(&keyword, KUWO_SEARCH_LIMIT).await {
        Ok(data) => data,
        Err(error) => return classify(error),
    };
    let items = match data["abslist"].as_array() {
        Some(items) => items,
        None => {
            return FallbackOutcome::Unavailable(format!(
                "酷我搜索响应缺少 abslist 字段（关键词 {}）",
                keyword
            ))
        }
    };

    // 3. 严格匹配 + 打分，选出唯一候选
    let mut best: Option<Candidate> = None;
    for item in items {
        let Some(candidate) = score_candidate(item, &meta, &formats, cap) else {
            continue;
        };
        let better = match best.as_ref() {
            Some(current) => candidate.score > current.score,
            None => true,
        };
        if better {
            best = Some(candidate);
        }
    }

    let Some(best) = best else {
        return FallbackOutcome::Unavailable(format!(
            "酷我搜索 {} 条候选里没有标题/歌手/时长三项全中的曲目（关键词 {}，时长 {}+/-{}s）",
            items.len(),
            keyword,
            meta.duration,
            MAX_DURATION_DELTA
        ));
    };
    if best.qualities.is_empty() {
        return FallbackOutcome::Unavailable(format!(
            "匹配曲目 {}「{}」在酷我没有扩展名兼容的明文音质",
            best.song_id, best.title
        ));
    }

    // 4. 按品质阶梯逐档下探取链
    let mut last_error = String::new();
    for quality in &best.qualities {
        match kuwo_link::get_download_link(client, &best.song_id, &quality.filename).await {
            Ok((url, key)) => {
                log::info!(
                    "歌曲 {}（{} - {}）从 QQ 音乐回退到酷我成功：{}（{}）",
                    song_mid,
                    meta.title,
                    meta.artist,
                    quality.filename,
                    quality.label
                );
                return FallbackOutcome::Linked(FallbackLink {
                    url,
                    key,
                    source: Platform::Kuwo,
                    quality: quality.label.clone(),
                });
            }
            Err(error) => {
                // 该档位不可用时酷我会返回「bitrate 不匹配」，继续下探下一档。
                if is_transient(&error) {
                    return FallbackOutcome::Transient(error);
                }
                log::warn!(
                    "回退取链失败（酷我 {} {} / {}）: {}",
                    best.song_id,
                    quality.filename,
                    best.title,
                    error
                );
                last_error = error;
            }
        }
    }

    FallbackOutcome::Unavailable(format!(
        "匹配曲目 {}「{}」在酷我的兼容音质全部取链失败: {}",
        best.song_id, best.title, last_error
    ))
}

/// 反查 QQ 歌曲信息：标题、歌手、时长（秒）与各品质声明大小。
async fn fetch_qq_song_meta(client: &Client, song_mid: &str) -> Result<SongMeta, String> {
    let body = json!({
        "comm": pc_comm(),
        QQ_DETAIL_MODULE: {
            "module": QQ_DETAIL_MODULE,
            "method": QQ_DETAIL_METHOD,
            "param": { "song_type": 0, "song_mid": song_mid }
        }
    });

    let resp = client
        .post(QQ_DETAIL_ENDPOINT)
        .header("Content-Type", "application/json")
        .header("Referer", "https://y.qq.com")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    let module = &data[QQ_DETAIL_MODULE];
    let code = module["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(format!("QQ 音乐歌曲详情返回异常（code={}）", code));
    }
    let info = &module["data"]["track_info"];
    if info.is_null() {
        return Err(format!("QQ 音乐歌曲详情缺少 track_info（{}）", song_mid));
    }

    // 多歌手用 `&` 连接，与酷我 `ARTIST` 字段的分隔符保持一致。
    let artist = info["singer"]
        .as_array()
        .map(|singers| {
            singers
                .iter()
                .filter_map(|singer| singer["name"].as_str())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join("&")
        })
        .unwrap_or_default();

    Ok(SongMeta {
        title: info["title"].as_str().unwrap_or_default().to_string(),
        artist,
        duration: info["interval"].as_u64().unwrap_or(0),
        size_128mp3: info["file"]["size_128mp3"].as_u64().unwrap_or(0),
        size_320mp3: info["file"]["size_320mp3"].as_u64().unwrap_or(0),
        size_flac: info["file"]["size_flac"].as_u64().unwrap_or(0),
    })
}

/// 反查到的 QQ 歌曲信息。
struct SongMeta {
    title: String,
    artist: String,
    /// 时长（秒）；0 表示接口未给，此时跳过时长校验。
    duration: u64,
    size_128mp3: u64,
    size_320mp3: u64,
    size_flac: u64,
}

/// 一个通过严格匹配的酷我候选曲目。
struct Candidate {
    song_id: String,
    title: String,
    score: i32,
    /// 已按扩展名与档位上限过滤、按比特率从高到低排序的明文品质。
    qualities: Vec<Quality>,
}

/// 酷我单条明文品质。
struct Quality {
    label: String,
    filename: String,
    /// 酷我声明的文件大小（字节）；`N_MINFO` 只精确到 0.01Mb。
    size: Option<u64>,
}

/// 给酷我原始条目打分；标题、歌手、时长三项全中才返回候选。
fn score_candidate(
    item: &Value,
    meta: &SongMeta,
    formats: &[&str],
    cap: Option<u32>,
) -> Option<Candidate> {
    let song = kuwo_parser::parse_song(item, "&")?;

    let title = song["title"].as_str().unwrap_or_default();
    if title.is_empty() || normalize(title) != normalize(&meta.title) {
        return None;
    }

    let artist = song["artist"].as_str().unwrap_or_default();
    if !artist_matches(&meta.artist, artist) {
        return None;
    }

    let duration = song["duration"].as_u64().unwrap_or(0);
    if meta.duration > 0 && duration > 0 && meta.duration.abs_diff(duration) > MAX_DURATION_DELTA {
        return None;
    }

    let qualities = select_qualities(&song["qualities"], formats, cap);
    let mut score = 100 + 40;
    if meta.duration > 0 && duration > 0 {
        // 时长完全一致比「差几秒」更可信，作为同分时的排序依据。
        score += if meta.duration == duration { 20 } else { 10 };
    }
    score += size_bonus(&qualities, meta);

    Some(Candidate {
        song_id: song["mid"].as_str().unwrap_or_default().to_string(),
        title: title.to_string(),
        score,
        qualities,
    })
}

/// 声明文件大小与 QQ 声明值吻合时加分。
///
/// 实测同一首歌在酷我与 QQ 是同一份母带（CDN 实际字节数完全相等），
/// 但酷我 `N_MINFO` 里的 size 只精确到 0.01Mb，因此按 1% 容差比较。
/// 该信号仅用于同分候选的排序，不作为硬门槛。
fn size_bonus(qualities: &[Quality], meta: &SongMeta) -> i32 {
    let declared = |filename: &str| -> u64 {
        match filename {
            "320.mp3" => meta.size_320mp3,
            "128.mp3" => meta.size_128mp3,
            "2000.flac" => meta.size_flac,
            _ => 0,
        }
    };
    for quality in qualities {
        let Some(size) = quality.size else { continue };
        let expected = declared(&quality.filename);
        if expected > 0 && expected.abs_diff(size) * 100 <= expected {
            return 5;
        }
    }
    0
}

/// 从酷我品质列表里挑出扩展名兼容、且不高于请求档位的明文品质，按比特率降序。
fn select_qualities(qualities: &Value, formats: &[&str], cap: Option<u32>) -> Vec<Quality> {
    let Some(list) = qualities.as_array() else {
        return Vec::new();
    };

    let mut selected: Vec<(u32, Quality)> = Vec::new();
    for entry in list {
        let Some(filename) = entry["filename"].as_str() else {
            continue;
        };
        // filename 约定为 `{bitrate}.{format}`（见 kuwo/parser.rs 的 build_qualities）。
        let Some((stem, format)) = filename.split_once('.') else {
            continue;
        };
        let Ok(bitrate) = stem.parse::<u32>() else {
            continue;
        };
        // 加密档（mflac/mgg）需要解密上下文，回退路径一律不用。
        if matches!(format, "mflac" | "mgg") || !formats.contains(&format) {
            continue;
        }
        if let Some(cap) = cap {
            if bitrate > cap {
                continue;
            }
        }
        selected.push((
            bitrate,
            Quality {
                label: entry["quality"].as_str().unwrap_or(format).to_string(),
                filename: filename.to_string(),
                size: entry["size"].as_u64(),
            },
        ));
    }

    selected.sort_by(|a, b| b.0.cmp(&a.0));
    selected.into_iter().map(|(_, quality)| quality).collect()
}

/// 任务文件名（`{QQ 品质前缀}{media_mid}.{ext}`）允许的回退容器。
///
/// 返回 `None` 表示该扩展名没有同类明文容器，禁止回退。
fn compatible_formats(filename: &str) -> Option<Vec<&'static str>> {
    let extension = filename.rsplit_once('.')?.1.to_ascii_lowercase();
    let formats = match extension.as_str() {
        "mp3" => vec!["mp3"],
        // QQ 的 flac/hires/母带都是 mflac（落盘时映射为 flac），酷我对应明文 flac。
        "mflac" | "flac" => vec!["flac"],
        "mgg" | "ogg" => vec!["ogg"],
        "m4a" | "aac" => vec!["aac", "m4a"],
        // ape 等容器在酷我没有匿名明文对应物。
        _ => return None,
    };
    Some(formats)
}

/// 任务文件名里 QQ 品质前缀对应的比特率上限；未知前缀返回 `None`（不设上限）。
///
/// 设上限是为了不把用户没要的更大文件塞给他，同时避免 `{quality}` 命名变量
/// 把实际音质写得比真实值更高。
fn requested_bitrate_cap(filename: &str) -> Option<u32> {
    let prefix = filename.get(0..4)?;
    let cap = match prefix {
        "C200" => 48,
        "O4M0" => 96,
        "C400" => 96,
        "M500" => 128,
        "C600" | "O6M0" => 192,
        "M800" => 320,
        "F0M0" => 2000,
        "RSM1" | "A000" => 4000,
        // 臻品全景声/母带（Q0M0/Q0M1/AIM0）不设上限，取酷我最高可用明文档。
        _ => return None,
    };
    Some(cap)
}

/// 归一化：只保留字母数字（含中日韩字符）并转小写，
/// 于是空格、全半角括号、《》、`·`、`-` 等装饰符差异不会影响比对。
///
/// 全角字母/数字先折成半角：同一首歌在 QQ 与酷我可能一个写 `ＬＩＶＥ`、
/// 一个写 `Live`，不折半角就会漏匹配。
fn normalize(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for character in text.chars() {
        let folded = match character as u32 {
            0xFF01..=0xFF5E => char::from_u32(character as u32 - 0xFEE0).unwrap_or(character),
            _ => character,
        };
        if folded.is_alphanumeric() {
            normalized.extend(folded.to_lowercase());
        }
    }
    normalized
}

/// 歌手匹配：任一歌手名归一化后互为子串即算匹配（多歌手任一对上即可）。
fn artist_matches(expected: &str, actual: &str) -> bool {
    let expected: Vec<String> = expected
        .split(['&', '/', ',', '，', '、', ';', '；'])
        .map(normalize)
        .filter(|name| !name.is_empty())
        .collect();
    let actual: Vec<String> = actual
        .split(['&', '/', ',', '，', '、', ';', '；'])
        .map(normalize)
        .filter(|name| !name.is_empty())
        .collect();

    expected.iter().any(|left| {
        actual.iter().any(|right| {
            left == right || left.contains(right.as_str()) || right.contains(left.as_str())
        })
    })
}

/// 临时性错误（网络/读取/解析）不应当作「找不到匹配」处理。
fn is_transient(error: &str) -> bool {
    error.starts_with("网络错误")
        || error.starts_with("读取响应失败")
        || error.starts_with("解析响应失败")
}

/// 把底层错误归类为回退结论。
fn classify(error: String) -> FallbackOutcome {
    if is_transient(&error) {
        FallbackOutcome::Transient(error)
    } else {
        FallbackOutcome::Unavailable(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(title: &str, artist: &str, duration: u64) -> SongMeta {
        SongMeta {
            title: title.to_string(),
            artist: artist.to_string(),
            duration,
            size_128mp3: 0,
            size_320mp3: 0,
            size_flac: 0,
        }
    }

    fn kuwo_item(title: &str, artist: &str, duration: u64, info: &str) -> Value {
        json!({
            "SONGNAME": title,
            "ARTIST": artist,
            "DURATION": duration,
            "MUSICRID": "MUSIC_228908",
            "N_MINFO": info
        })
    }

    const FULL_INFO: &str = "level:ff,bitrate:20900,format:mflac,size:178.32Mb;\
level:ff,bitrate:2000,format:flac,size:52.83Mb;\
level:p,bitrate:320,format:mp3,size:10.29Mb;\
level:h,bitrate:128,format:mp3,size:4.12Mb";

    #[test]
    fn title_and_artist_and_duration_must_all_match() {
        let info = FULL_INFO;
        let target = meta("晴天", "周杰伦", 269);

        // 三项全中
        assert!(score_candidate(
            &kuwo_item("晴天", "周杰伦", 269, info),
            &target,
            &["mp3"],
            Some(320)
        )
        .is_some());
        // 标题是 KTV 版（归一化后不相等）
        assert!(score_candidate(
            &kuwo_item("晴天 (KTV版伴奏)", "周杰伦", 269, info),
            &target,
            &["mp3"],
            Some(320)
        )
        .is_none());
        // 翻唱歌手
        assert!(score_candidate(
            &kuwo_item("晴天", "某翻唱歌手", 269, info),
            &target,
            &["mp3"],
            Some(320)
        )
        .is_none());
        // 时长差 4 秒 > 3 秒上限
        assert!(score_candidate(
            &kuwo_item("晴天", "周杰伦", 273, info),
            &target,
            &["mp3"],
            Some(320)
        )
        .is_none());
    }

    #[test]
    fn duration_unknown_on_either_side_is_tolerated() {
        let info = FULL_INFO;
        // 酷我未给时长
        assert!(score_candidate(
            &kuwo_item("晴天", "周杰伦", 0, info),
            &meta("晴天", "周杰伦", 269),
            &["mp3"],
            Some(320)
        )
        .is_some());
        // QQ 详情未给时长
        assert!(score_candidate(
            &kuwo_item("晴天", "周杰伦", 269, info),
            &meta("晴天", "周杰伦", 0),
            &["mp3"],
            Some(320)
        )
        .is_some());
    }

    #[test]
    fn multi_artist_matches_when_any_name_intersects() {
        assert!(artist_matches("周杰伦&方文山", "方文山"));
        assert!(artist_matches("周杰伦", "周杰伦&袁咏琳"));
        assert!(!artist_matches("周杰伦", "陈奕迅"));
    }

    #[test]
    fn normalize_ignores_punctuation_case_and_width() {
        assert_eq!(normalize("晴天 (Live)"), normalize("晴天（ＬＩＶＥ）"));
        assert_eq!(normalize("Hello, World!"), "helloworld");
        assert_ne!(normalize("晴天"), normalize("晴天娃娃"));
    }

    #[test]
    fn qualities_exclude_encrypted_and_respect_format_and_cap() {
        let qualities = Value::Array(kuwo_parser::build_qualities(FULL_INFO));
        let mp3 = select_qualities(&qualities, &["mp3"], None);
        assert_eq!(
            mp3.iter().map(|q| q.filename.as_str()).collect::<Vec<_>>(),
            vec!["320.mp3", "128.mp3"]
        );
        // 加密档与其它容器一律排除
        let flac = select_qualities(&qualities, &["flac"], None);
        assert_eq!(
            flac.iter().map(|q| q.filename.as_str()).collect::<Vec<_>>(),
            vec!["2000.flac"]
        );
        // 上限过滤掉用户没要的更高档位
        let capped = select_qualities(&qualities, &["mp3"], Some(128));
        assert_eq!(
            capped
                .iter()
                .map(|q| q.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["128.mp3"]
        );
    }

    #[test]
    fn compatible_formats_follows_task_extension() {
        assert_eq!(
            compatible_formats("M800003Qui1q2u1Zho.mp3"),
            Some(vec!["mp3"])
        );
        assert_eq!(
            compatible_formats("F0M0003Qui1q2u1Zho.mflac"),
            Some(vec!["flac"])
        );
        assert_eq!(
            compatible_formats("O6M0003Qui1q2u1Zho.mgg"),
            Some(vec!["ogg"])
        );
        assert_eq!(compatible_formats("A000003Qui1q2u1Zho.ape"), None);
    }

    #[test]
    fn bitrate_cap_comes_from_quality_prefix() {
        assert_eq!(requested_bitrate_cap("M500003Qui1q2u1Zho.mp3"), Some(128));
        assert_eq!(requested_bitrate_cap("M800003Qui1q2u1Zho.mp3"), Some(320));
        assert_eq!(
            requested_bitrate_cap("F0M0003Qui1q2u1Zho.mflac"),
            Some(2000)
        );
        assert_eq!(
            requested_bitrate_cap("RSM1003Qui1q2u1Zho.mflac"),
            Some(4000)
        );
        // 臻品母带/全景声不设上限
        assert_eq!(requested_bitrate_cap("AIM0003Qui1q2u1Zho.mflac"), None);
        assert_eq!(requested_bitrate_cap("Q0M0003Qui1q2u1Zho.mflac"), None);
    }

    #[test]
    fn size_match_only_counts_within_one_percent() {
        let qualities = vec![Quality {
            label: "320kmp3".to_string(),
            filename: "320.mp3".to_string(),
            size: Some(10_792_943),
        }];

        let mut sizes = meta("晴天", "周杰伦", 269);
        sizes.size_320mp3 = 10_792_943;
        assert_eq!(size_bonus(&qualities, &sizes), 5, "完全相等应加分");

        // 酷我 N_MINFO 只精确到 0.01Mb，1% 以内的偏差仍算吻合
        sizes.size_320mp3 = 10_750_000;
        assert_eq!(size_bonus(&qualities, &sizes), 5);

        sizes.size_320mp3 = 9_000_000;
        assert_eq!(size_bonus(&qualities, &sizes), 0, "明显不同的文件不加分");

        let unknown = meta("晴天", "周杰伦", 269);
        assert_eq!(size_bonus(&qualities, &unknown), 0, "QQ 未声明大小时不加分");
    }

    #[tokio::test]
    async fn kuwo_platform_has_no_fallback_source() {
        let outcome =
            fetch_from_other_sources(&Client::new(), Platform::Kuwo, "228908", "320.mp3").await;
        assert!(matches!(outcome, FallbackOutcome::Unavailable(_)));
    }
}
