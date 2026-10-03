//! 内置多音源回退：主平台取链失败时，用歌曲元信息在其它内置音源里重新解析直链。
//!
//! # 为什么需要反查歌曲信息
//!
//! `DownloadLinkProvider::fetch` 只拿得到 `platform` / `song_mid` / `filename` 三个参数
//! （trait 签名被冻结：`app/src-tauri/src/adapters/tauri_download_host.rs` 是纯委托实现，
//! 改动会波及该文件），所以回退必须自己反查标题、歌手与时长。
//! QQ 音乐 / 酷狗 / 网易云三家的匿名详情接口都一次给足这三项，用来在酷我搜索结果里做严格匹配
//! （字段契约见 `_dev/unlock-probe/meta-probe.mjs` 的实测输出）。
//!
//! # 覆盖范围
//!
//! 已接入回退的源站：**QQ 音乐、酷狗、网易云、咪咕**。四家都在匿名态对付费/VIP 曲目
//! 确定性拒绝（见各 `platforms/*/link.rs` 的实测注释），而酷我匿名仍给完整明文直链，
//! 因此「付费曲目自动换源」在此实现。
//!
//! 酷我可用的档位经 live 实测（`_dev/unlock-probe/fallback-e2e.mjs`）：320k mp3、
//! 128k mp3 与 2000k flac 都能取到真实音频字节（Range 校验：`ID3`/`fLaC` 魔数 +
//! 完整体积），即无损任务也能回退，不必降级成 mp3。哔哩哔哩没有可用的元信息反查接口，
//! 保留主平台原始错误。
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

use reqwest::{Client, Url};
use serde_json::{json, Value};

use crate::platforms::kugou::parser as kugou_parser;
use crate::platforms::kuwo::{link as kuwo_link, parser as kuwo_parser, search as kuwo_search};
use crate::platforms::migu;
use crate::platforms::netease::link::parse_song_id as parse_netease_song_id;
use crate::platforms::qqmusic::search::pc_comm;
use crate::platforms::Platform;

/// 确定性的「找不到可回退音源」文案，直接展示给用户。
pub(crate) const NO_MATCH_MESSAGE: &str = "未在内置音源找到匹配的歌曲：QQ 音乐需要登录或该曲目受限";

/// QQ 歌曲详情接口（匿名可用），用于反查标题/歌手/时长。
const QQ_DETAIL_ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const QQ_DETAIL_MODULE: &str = "music.pf_song_detail_svr";
const QQ_DETAIL_METHOD: &str = "get_song_detail_yqq";

/// 酷狗 `getSongInfo`（匿名可用）：付费曲目虽不给直链，但**照常返回标题/歌手/时长与各档大小**，
/// 实测见 `_dev/unlock-probe/meta-probe.mjs`（VIP 曲目 `status=0` 仍带 `songName`/`singerName`/`extra`）。
const KUGOU_SONG_INFO_ENDPOINT: &str = "https://m.kugou.com/app/i/getSongInfo.php";
/// 该接口按移动端返回 JSON，与 `platforms/kugou/link.rs` 使用同一组请求头。
const KUGOU_MOBILE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 13; V2227A Build/TP1A.220624.014) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/114.0.0.0 Mobile Safari/537.36";
const KUGOU_MOBILE_REFERER: &str = "https://m.kugou.com/";

/// 网易云歌曲详情接口（匿名可用）。
const NETEASE_DETAIL_ENDPOINT: &str = "https://music.163.com/api/song/detail";
const NETEASE_DETAIL_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const NETEASE_DETAIL_COOKIE: &str = "appver=8.7.01; os=pc";

/// 咪咕歌曲详情接口（匿名可用，需带 App 的 `channel` 头，见 `platforms/migu/mod.rs`）。
///
/// 实测 VIP 曲目上 `listenSong.do` 只回「暂不提供试听地址」，而本接口照常返回
/// `songName` / `singer` / `length`（`00:04:30`）与 `newRateFormats` 各档声明大小，
/// 因此咪咕也能接入回退（探测见 `_dev/unlock-probe/migu-meta-probe.mjs`）。
const MIGU_RESOURCE_ENDPOINT: &str =
    "https://app.c.nf.migu.cn/MIGUM2.0/v1.0/content/resourceinfo.do";

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
        Platform::Kugou => "酷狗",
        Platform::Netease => "网易云",
        Platform::Bilibili => "哔哩哔哩",
        Platform::Migu => "咪咕",
        Platform::Script(_) => "自定义音源",
    }
}

/// 主平台取链失败后的内置回退入口。
///
/// 当前内置的匿名可用明文直链只有酷我（实测酷狗/咪咕/网易云的下载接口都要会员或签名），
/// 因此所有可回退源站都指向酷我，区别只在「用哪个接口反查歌曲信息」。
/// 无元信息反查能力的平台（咪咕、哔哩哔哩）保留主平台的原始错误，
/// 跨平台自动改名落地比失败更糟。
pub(crate) async fn fetch_from_other_sources(
    client: &Client,
    platform: Platform,
    song_mid: &str,
    filename: &str,
) -> FallbackOutcome {
    match platform {
        // 四家源站的付费/VIP 曲目在匿名态都确定性失败，而酷我匿名仍给完整明文直链。
        Platform::QqMusic | Platform::Kugou | Platform::Netease | Platform::Migu => {
            fallback_to_kuwo(client, platform, song_mid, filename).await
        }
        // 酷我自身没有可替代的内置音源，保留主平台错误。
        Platform::Kuwo => FallbackOutcome::Unavailable("酷我任务没有可用的回退音源".to_string()),
        // 自定义音源脚本本身就是用户指定的来源，内置回退不接手。
        Platform::Script(_) => {
            FallbackOutcome::Unavailable("自定义音源任务不做内置回退".to_string())
        }
        other => {
            FallbackOutcome::Unavailable(format!("{}任务没有可用的回退音源", platform_label(other)))
        }
    }
}

/// 回退失败时给用户看的最终文案。
///
/// - QQ 音乐的原始错误（如 104003「无法获取下载链接」）对用户没有指导意义，
///   换成原有的「需要登录或曲目受限 + 没找到替代」说明；
/// - 酷狗/网易云的原始错误（付费/VIP 受限）本身是准确的，追加一句回退结论即可。
pub(crate) fn no_match_message(platform: Platform, primary_error: &str) -> String {
    if matches!(platform, Platform::QqMusic) {
        return NO_MATCH_MESSAGE.to_string();
    }

    format!("{primary_error}；已在其它内置音源搜索，未找到可替代的匹配版本")
}

/// 任务文件名是否在请求无损容器。
fn is_lossless_filename(filename: &str) -> bool {
    filename
        .rsplit_once('.')
        .map(|(_, extension)| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "flac" | "mflac" | "ape" | "wav"
            )
        })
        .unwrap_or(false)
}

/// 源站 → 酷我音乐的回退链路（QQ 音乐 / 酷狗 / 网易云共用）。
async fn fallback_to_kuwo(
    client: &Client,
    platform: Platform,
    song_mid: &str,
    filename: &str,
) -> FallbackOutcome {
    let formats = match compatible_formats(filename) {
        Some(formats) => formats,
        None => {
            return FallbackOutcome::Unavailable(format!(
                "任务文件名 {} 的扩展名在酷我没有同类明文容器",
                filename
            ))
        }
    };
    let cap = requested_bitrate_cap(platform, filename);

    // 1. 反查歌曲信息（标题/歌手/时长）
    let meta = match fetch_song_meta(client, platform, song_mid).await {
        Ok(meta) => meta,
        Err(error) => return classify(error),
    };
    if meta.title.is_empty() || meta.artist.is_empty() {
        return FallbackOutcome::Unavailable(format!(
            "{}歌曲详情缺少标题或歌手（{}）",
            platform_label(platform),
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
        // 曲库未收录该曲的无损档时，酷我只列 mp3；给用户一条能立刻走通的退路。
        let hint = if is_lossless_filename(filename) {
            "（该曲在酷我未收录无损档，可改选 320k 音质后重试）"
        } else {
            ""
        };
        return FallbackOutcome::Unavailable(format!(
            "匹配曲目 {}「{}」在酷我没有扩展名兼容的明文音质{}",
            best.song_id, best.title, hint
        ));
    }

    // 4. 按品质阶梯逐档下探取链
    let mut last_error = String::new();
    for quality in &best.qualities {
        match kuwo_link::get_download_link(client, &best.song_id, &quality.filename).await {
            Ok((url, key)) => {
                log::info!(
                    "歌曲 {}（{} - {}）从{}回退到酷我成功：{}（{}）",
                    song_mid,
                    meta.title,
                    meta.artist,
                    platform_label(platform),
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

/// 按源站分派歌曲信息反查。
async fn fetch_song_meta(
    client: &Client,
    platform: Platform,
    song_mid: &str,
) -> Result<SongMeta, String> {
    match platform {
        Platform::QqMusic => fetch_qq_song_meta(client, song_mid).await,
        Platform::Kugou => fetch_kugou_song_meta(client, song_mid).await,
        Platform::Netease => fetch_netease_song_meta(client, song_mid).await,
        Platform::Migu => fetch_migu_song_meta(client, song_mid).await,
        other => Err(format!(
            "{}没有内置的歌曲信息反查实现",
            platform_label(other)
        )),
    }
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

/// 反查酷狗歌曲信息（标题、歌手、时长与各档声明大小）。
///
/// 用 128k hash 调 `getSongInfo`：**付费/VIP 曲目同样返回完整元信息**（只有 `url` 为空、
/// `fileSize` 为 0），所以这个接口既能给免费曲也能给受限曲做回退前的匹配依据。
/// 实测样本见 `_dev/unlock-probe/meta-probe.mjs`：VIP《晴天》返回 `songName=晴天`、
/// `singerName=周杰伦`、`extra.320timelength=269000`、`extra.320filesize=10792943`。
async fn fetch_kugou_song_meta(client: &Client, song_mid: &str) -> Result<SongMeta, String> {
    let mid = kugou_parser::parse_mid(song_mid)?;
    let hash = mid.hash_128.trim();
    if hash.is_empty() {
        return Err(format!("酷狗歌曲标识缺少 128k hash: {}", song_mid));
    }

    let url = Url::parse_with_params(
        KUGOU_SONG_INFO_ENDPOINT,
        &[("cmd", "playInfo"), ("hash", hash)],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))?;

    let response = client
        .get(url)
        .header("User-Agent", KUGOU_MOBILE_USER_AGENT)
        .header("Referer", KUGOU_MOBILE_REFERER)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    let title = data["songName"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();

    // `timeLength` 在受限曲目上是 0，真实时长在各档的 `extra.*timelength`（毫秒）里。
    let duration_ms = ["320timelength", "128timelength", "sqtimelength"]
        .iter()
        .filter_map(|key| data["extra"][*key].as_u64())
        .find(|value| *value > 0)
        .or_else(|| {
            data["timeLength"]
                .as_u64()
                .filter(|value| *value > 0)
                .map(|seconds| seconds * 1000)
        })
        .unwrap_or(0);

    Ok(SongMeta {
        title,
        artist: kugou_artist(&data),
        duration: duration_ms / 1000,
        size_128mp3: data["extra"]["128filesize"].as_u64().unwrap_or(0),
        size_320mp3: data["extra"]["320filesize"].as_u64().unwrap_or(0),
        size_flac: data["extra"]["sqfilesize"].as_u64().unwrap_or(0),
    })
}

/// 酷狗歌手名：优先 `singerName`（多歌手用 `、` 分隔），缺失时退回 `authors` 列表。
fn kugou_artist(data: &Value) -> String {
    let singer_name = data["singerName"].as_str().unwrap_or_default().trim();
    if !singer_name.is_empty() {
        return singer_name.to_string();
    }

    data["authors"]
        .as_array()
        .map(|authors| {
            authors
                .iter()
                .filter_map(|author| author["author_name"].as_str())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join("&")
        })
        .unwrap_or_default()
}

/// 反查网易云歌曲信息：`api/song/detail` 匿名可用，
/// `hMusic`/`lMusic` 分别给出 320k/128k 的声明字节数（实测《晴天》320k = 10792794）。
async fn fetch_netease_song_meta(client: &Client, song_mid: &str) -> Result<SongMeta, String> {
    let song_id = parse_netease_song_id(song_mid)?;
    let ids = format!("[{}]", song_id);
    let url = Url::parse_with_params(NETEASE_DETAIL_ENDPOINT, &[("ids", ids.as_str())])
        .map_err(|e| format!("URL 构建失败: {}", e))?;

    let response = client
        .get(url)
        .header("User-Agent", NETEASE_DETAIL_USER_AGENT)
        .header("Referer", "https://music.163.com/")
        .header("Cookie", NETEASE_DETAIL_COOKIE)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    let song = data["songs"]
        .as_array()
        .and_then(|songs| songs.first())
        .ok_or_else(|| format!("网易云歌曲详情为空（{}）", song_id))?;

    let artist = song["artists"]
        .as_array()
        .map(|artists| {
            artists
                .iter()
                .filter_map(|artist| artist["name"].as_str())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join("&")
        })
        .unwrap_or_default();

    Ok(SongMeta {
        title: song["name"].as_str().unwrap_or_default().trim().to_string(),
        artist,
        // `duration` 单位是毫秒。
        duration: song["duration"].as_u64().unwrap_or(0) / 1000,
        size_128mp3: song["lMusic"]["size"].as_u64().unwrap_or(0),
        size_320mp3: song["hMusic"]["size"].as_u64().unwrap_or(0),
        size_flac: song["sqMusic"]["size"]
            .as_u64()
            .or_else(|| song["hrMusic"]["size"].as_u64())
            .unwrap_or(0),
    })
}

/// 反查咪咕歌曲信息：`resourceinfo.do` 对 VIP 曲目同样返回完整元信息。
///
/// `song_mid` 契约是 `{copyrightId}|{contentId}`（见 `platforms/migu/parser.rs`），
/// 反查只需要前段。
async fn fetch_migu_song_meta(client: &Client, song_mid: &str) -> Result<SongMeta, String> {
    let copyright_id = song_mid
        .split_once('|')
        .map(|(copyright_id, _)| copyright_id.trim())
        .filter(|copyright_id| !copyright_id.is_empty())
        .ok_or_else(|| format!("无效的咪咕歌曲标识: {}", song_mid))?;

    let url = Url::parse_with_params(
        MIGU_RESOURCE_ENDPOINT,
        &[("copyrightId", copyright_id), ("resourceType", "2")],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))?;

    let response = migu::app_request(client.get(url))
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    let resource = data["resource"]
        .as_array()
        .and_then(|resources| resources.first())
        .ok_or_else(|| format!("咪咕歌曲详情为空（{}）", copyright_id))?;

    let (size_128mp3, size_320mp3, size_flac) = migu_sizes(resource);

    Ok(SongMeta {
        title: resource["songName"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string(),
        artist: resource["singer"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string(),
        duration: migu_duration(resource["length"].as_str().unwrap_or_default()),
        size_128mp3,
        size_320mp3,
        size_flac,
    })
}

/// 咪咕 `length` 形如 `00:04:30`（时:分:秒），少数条目给 `04:30`（分:秒），统一转成秒。
fn migu_duration(length: &str) -> u64 {
    let parts: Vec<u64> = length
        .split(':')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect();
    match parts.as_slice() {
        [hours, minutes, seconds] => hours * 3600 + minutes * 60 + seconds,
        [minutes, seconds] => minutes * 60 + seconds,
        [seconds] => *seconds,
        _ => 0,
    }
}

/// 从 `newRateFormats` 取各档声明字节数：`PQ`=128k、`HQ`=320k、`SQ`=无损。
///
/// 咪咕把 Android 与 iOS 的字节数分开给（`androidSize` / `iosSize`），落盘的是 Android 侧。
fn migu_sizes(resource: &Value) -> (u64, u64, u64) {
    let formats = resource["newRateFormats"].as_array();
    let size_of = |wanted: &str| -> u64 {
        formats
            .and_then(|list| {
                list.iter()
                    .find(|entry| entry["formatType"].as_str() == Some(wanted))
            })
            .map(migu_size)
            .unwrap_or(0)
    };
    (
        size_of("PQ"),
        size_of("HQ"),
        size_of("SQ"),
    )
}

/// 咪咕单条 `newRateFormats` 的字节数（`androidSize` 优先，兼容字符串与数字两种写法）。
fn migu_size(entry: &Value) -> u64 {
    entry["androidSize"]
        .as_u64()
        .or_else(|| entry["androidSize"].as_str().and_then(|v| v.parse().ok()))
        .or_else(|| entry["size"].as_u64())
        .or_else(|| entry["size"].as_str().and_then(|v| v.parse().ok()))
        .unwrap_or(0)
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

/// 声明文件大小与源站声明值吻合时加分。
///
/// 实测同一首歌在酷我与 QQ/酷狗/网易云是同一份母带（CDN 实际字节数几乎相等，
/// 差异只在各站元信息的取整），但酷我 `N_MINFO` 里的 size 只精确到 0.01Mb，
/// 因此按 1% 容差比较。该信号仅用于同分候选的排序，不作为硬门槛。
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

/// 任务文件名（`{品质编码}{media_mid}.{ext}`）允许的回退容器。
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

/// 任务文件名对应的比特率上限；`None` 表示不设上限。
///
/// 设上限是为了不把用户没要的更大文件塞给他，同时避免 `{quality}` 命名变量
/// 把实际音质写得比真实值更高。
///
/// 各源的档位编码方式不同：
/// - QQ 音乐把品质编码在 4 字符前缀里（`M800…`、`F0M0…`）；
/// - 酷狗/网易云沿用酷我的 `{bitrate}.{format}` 约定（`128.mp3` / `320.mp3` / `2000.flac`）；
/// - 咪咕用音质档标识（`PQ.mp3` / `HQ.mp3` / `SQ.flac`）。
fn requested_bitrate_cap(platform: Platform, filename: &str) -> Option<u32> {
    match platform {
        Platform::QqMusic => qq_bitrate_cap(filename),
        Platform::Kugou | Platform::Netease => stem_bitrate(filename),
        Platform::Migu => migu_bitrate_cap(filename),
        _ => None,
    }
}

/// 任务文件名里 QQ 品质前缀对应的比特率上限；未知前缀返回 `None`（不设上限）。
fn qq_bitrate_cap(filename: &str) -> Option<u32> {
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

/// 咪咕音质档标识（`PQ.mp3`/`HQ.mp3`/`SQ.flac`）对应的比特率上限。
///
/// 未知档位（`LQ`、臻品音质等）返回 `None`：不设上限，取酷我最高可用明文档。
fn migu_bitrate_cap(filename: &str) -> Option<u32> {
    let stem = filename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(filename);
    match stem.to_ascii_uppercase().as_str() {
        "PQ" => Some(128),
        "HQ" => Some(320),
        "SQ" => Some(2000),
        _ => None,
    }
}

/// 从 `{bitrate}.{format}` 的 stem 取码率，兼容 `128kmp3` 这类契约标签形式。
fn stem_bitrate(filename: &str) -> Option<u32> {
    let stem = filename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(filename);
    let digits: String = stem
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();

    digits.parse::<u32>().ok().filter(|bitrate| *bitrate > 0)
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
        let cap = |filename: &str| requested_bitrate_cap(Platform::QqMusic, filename);
        assert_eq!(cap("M500003Qui1q2u1Zho.mp3"), Some(128));
        assert_eq!(cap("M800003Qui1q2u1Zho.mp3"), Some(320));
        assert_eq!(cap("F0M0003Qui1q2u1Zho.mflac"), Some(2000));
        assert_eq!(cap("RSM1003Qui1q2u1Zho.mflac"), Some(4000));
        // 臻品母带/全景声不设上限
        assert_eq!(cap("AIM0003Qui1q2u1Zho.mflac"), None);
        assert_eq!(cap("Q0M0003Qui1q2u1Zho.mflac"), None);
    }

    #[test]
    fn kugou_and_netease_caps_come_from_filename_stem() {
        // 酷狗/网易云沿用 `{bitrate}.{format}` 约定，码率就是 stem。
        for platform in [Platform::Kugou, Platform::Netease] {
            assert_eq!(
                requested_bitrate_cap(platform, "128.mp3"),
                Some(128),
                "{platform:?} 128k 档上限"
            );
            assert_eq!(
                requested_bitrate_cap(platform, "320.mp3"),
                Some(320),
                "{platform:?} 320k 档上限"
            );
        }
        assert_eq!(
            requested_bitrate_cap(Platform::Kugou, "2000.flac"),
            Some(2000)
        );
        assert_eq!(
            requested_bitrate_cap(Platform::Kugou, "320kmp3.mp3"),
            Some(320),
            "兼容历史契约标签形式"
        );
        // 无回退源站的平台不设上限（回退也不会被调用）
        assert_eq!(requested_bitrate_cap(Platform::Migu, "128.mp3"), None);
    }

    #[test]
    fn kugou_artist_prefers_singer_name_and_falls_back_to_authors() {
        // 实测 VIP《晴天》响应片段
        let vip = json!({
            "songName": "晴天",
            "singerName": "周杰伦",
            "authors": [{ "author_name": "周杰伦" }]
        });
        assert_eq!(kugou_artist(&vip), "周杰伦");

        let multi = json!({ "singerName": "周杰伦、袁咏琳" });
        assert_eq!(kugou_artist(&multi), "周杰伦、袁咏琳");

        let no_singer = json!({ "authors": [{ "author_name": "A" }, { "author_name": "B" }] });
        assert_eq!(kugou_artist(&no_singer), "A&B");

        assert_eq!(kugou_artist(&json!({})), "");
    }

    #[test]
    fn lossless_filenames_are_detected() {
        assert!(is_lossless_filename("2000.flac"));
        assert!(is_lossless_filename("F0M0003Qui1q2u1Zho.mflac"));
        assert!(is_lossless_filename("A000003Qui1q2u1Zho.ape"));
        assert!(!is_lossless_filename("320.mp3"));
        assert!(!is_lossless_filename("320kmp3.mp3"));
        assert!(!is_lossless_filename("no-extension"));
    }

    #[test]
    fn no_match_message_keeps_platform_error_and_appends_fallback_note() {
        // QQ 保留原有文案（其原始错误对用户没有指导意义）
        assert_eq!(
            no_match_message(Platform::QqMusic, "平台拒绝: 104003"),
            NO_MATCH_MESSAGE
        );

        // 酷狗保留付费/VIP 判定，并说明已尝试回退
        let kugou = no_match_message(
            Platform::Kugou,
            "酷狗音乐：该歌曲为付费/VIP 曲目，匿名无法获取下载链接",
        );
        assert!(kugou.starts_with("酷狗音乐：该歌曲为付费/VIP 曲目"));
        assert!(kugou.contains("已在其它内置音源搜索"));
        assert!(!kugou.contains("改选 320k"), "原始错误准确时不该附加音质建议");
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

    #[tokio::test]
    async fn kugou_netease_and_migu_route_into_the_kuwo_fallback() {
        // 非法 mid 在发起网络请求之前就会失败，因此这个测试不联网：
        // 它证明酷狗/网易云/咪咕确实进入了回退链路（而不是被「没有可用的回退音源」直接挡掉）。
        let client = Client::new();

        match fetch_from_other_sources(&client, Platform::Kugou, "not-a-hash", "128.mp3").await {
            FallbackOutcome::Unavailable(reason) => {
                assert!(reason.contains("无效的酷狗歌曲标识"), "{reason}")
            }
            _ => panic!("酷狗非法 mid 应当是确定性失败"),
        }

        match fetch_from_other_sources(&client, Platform::Netease, "abc", "128.mp3").await {
            FallbackOutcome::Unavailable(reason) => {
                assert!(reason.contains("网易云歌曲 ID 无效"), "{reason}")
            }
            _ => panic!("网易云非法 ID 应当是确定性失败"),
        }

        match fetch_from_other_sources(&client, Platform::Migu, "no-separator", "PQ.mp3").await {
            FallbackOutcome::Unavailable(reason) => {
                assert!(reason.contains("无效的咪咕歌曲标识"), "{reason}")
            }
            _ => panic!("咪咕非法 mid 应当是确定性失败"),
        }
    }

    #[test]
    fn migu_duration_parses_hh_mm_ss_and_mm_ss() {
        assert_eq!(migu_duration("00:04:30"), 270);
        assert_eq!(migu_duration("04:30"), 270);
        assert_eq!(migu_duration("1:02:03"), 3723);
        assert_eq!(migu_duration(""), 0);
        assert_eq!(migu_duration("abc"), 0);
    }

    #[test]
    fn migu_sizes_read_new_rate_formats() {
        let resource = json!({
            "newRateFormats": [
                { "formatType": "PQ", "size": "4317311", "fileType": "mp3" },
                { "formatType": "HQ", "size": "10792962", "androidSize": "10792962", "fileType": "mp3" },
                { "formatType": "SQ", "size": "31529675", "androidSize": "31529675", "fileType": "flac" }
            ]
        });
        assert_eq!(migu_sizes(&resource), (4_317_311, 10_792_962, 31_529_675));
        assert_eq!(migu_sizes(&json!({})), (0, 0, 0));
    }

    #[test]
    fn migu_caps_come_from_tone_flag() {
        assert_eq!(requested_bitrate_cap(Platform::Migu, "PQ.mp3"), Some(128));
        assert_eq!(requested_bitrate_cap(Platform::Migu, "HQ.mp3"), Some(320));
        assert_eq!(requested_bitrate_cap(Platform::Migu, "SQ.flac"), Some(2000));
        // 未知档位不设上限，取酷我最高可用明文档。
        assert_eq!(requested_bitrate_cap(Platform::Migu, "ZQ.flac"), None);
    }

    // ------------------------------------------------------------------
    // 真实网络端到端（默认 `#[ignore]`）
    //
    // 这些测试直接跑生产代码路径 `fetch_from_other_sources`，再用一次 Range 请求
    // 校验拿到的直链确实是完整音频（比对源站声明的字节数），而不是试听片段。
    //
    //   cargo test -p hotdownloader-core --lib -- --ignored --nocapture live_
    //
    // 需要源站在线，因此不进默认测试集。
    // ------------------------------------------------------------------

    /// 用 `Range: bytes=0-127` 取直链，返回 `Content-Range` 里的完整长度。
    async fn ranged_total(client: &Client, url: &str) -> u64 {
        let response = client
            .get(url)
            .header(reqwest::header::RANGE, "bytes=0-127")
            .send()
            .await
            .expect("直链请求失败");
        assert_eq!(
            response.status().as_u16(),
            206,
            "直链应支持 Range 并返回 206"
        );
        let content_range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .expect("直链响应缺少 Content-Range")
            .to_string();
        content_range
            .rsplit('/')
            .next()
            .and_then(|total| total.trim().parse().ok())
            .unwrap_or_else(|| panic!("Content-Range 无法解析: {content_range}"))
    }

    fn expect_linked(outcome: FallbackOutcome, platform: Platform) -> FallbackLink {
        match outcome {
            FallbackOutcome::Linked(link) => {
                assert_eq!(link.source, Platform::Kuwo, "回退音源应为酷我");
                link
            }
            FallbackOutcome::Unavailable(reason) => {
                panic!("{} 付费曲目应当回退成功，实际 Unavailable: {reason}", platform_label(platform))
            }
            FallbackOutcome::Transient(reason) => {
                panic!("{} 付费曲目回退遇到临时错误: {reason}", platform_label(platform))
            }
        }
    }

    /// 酷狗付费曲目（周杰伦《晴天》）→ 酷我 320k，字节数应与酷狗 `320filesize` 一致。
    #[tokio::test]
    #[ignore = "真实网络：需要酷狗与酷我接口在线"]
    async fn live_kugou_paid_track_falls_back_to_kuwo_320() {
        let client = Client::new();
        let mid = "b3a52a7a958bf0aed0ebfba2e9a818b7|1b56126a8a03924f1dd066259c095cbc|0a69169202de95aaf24a9944ccf0730d|966846";

        let link = expect_linked(
            fetch_from_other_sources(&client, Platform::Kugou, mid, "320.mp3").await,
            Platform::Kugou,
        );
        let total = ranged_total(&client, &link.url).await;
        assert_eq!(total, 10_792_943, "酷狗声明的 320filesize 应逐字节一致");
        println!("酷狗 320.mp3 → {} 字节（{}）", total, link.quality);
    }

    /// 酷狗付费曲目的无损档 → 酷我 2000k flac（对映酷狗 Hi-Res 档 55,397,039 字节）。
    #[tokio::test]
    #[ignore = "真实网络：需要酷狗与酷我接口在线"]
    async fn live_kugou_paid_track_falls_back_to_kuwo_flac() {
        let client = Client::new();
        let mid = "b3a52a7a958bf0aed0ebfba2e9a818b7|1b56126a8a03924f1dd066259c095cbc|0a69169202de95aaf24a9944ccf0730d|966846";

        let link = expect_linked(
            fetch_from_other_sources(&client, Platform::Kugou, mid, "2000.flac").await,
            Platform::Kugou,
        );
        let total = ranged_total(&client, &link.url).await;
        assert_eq!(total, 55_397_039, "酷狗声明的 highfilesize 应逐字节一致");
        println!("酷狗 2000.flac → {} 字节（{}）", total, link.quality);
    }

    /// 网易云付费曲目（《晴天》186016）→ 酷我 320k（hMusic.size 10,792,794）。
    #[tokio::test]
    #[ignore = "真实网络：需要网易云与酷我接口在线"]
    async fn live_netease_paid_track_falls_back_to_kuwo_320() {
        let client = Client::new();

        let link = expect_linked(
            fetch_from_other_sources(&client, Platform::Netease, "186016", "320.mp3").await,
            Platform::Netease,
        );
        let total = ranged_total(&client, &link.url).await;
        // 源站声明与真实文件相差几个字节（网易云 hMusic.size 10792794 / 实测 10792943）。
        let delta = (total as i64 - 10_792_794).abs();
        assert!(
            delta * 1000 < 10_792_794,
            "字节数应与网易云 hMusic.size 相差不到 0.1%，实际 {total}"
        );
        println!("网易云 320.mp3 → {} 字节（{}）", total, link.quality);
    }

    /// 咪咕 VIP 独占曲目（《晴天》copyrightId 60054701923）→ 酷我 320k。
    ///
    /// 咪咕 `listenSong.do` 对这首只回「暂不提供试听地址」，但 `resourceinfo.do`
    /// 照常给出元信息，因此回退链路仍然成立。
    #[tokio::test]
    #[ignore = "真实网络：需要咪咕与酷我接口在线"]
    async fn live_migu_vip_track_falls_back_to_kuwo_320() {
        let client = Client::new();

        let link = expect_linked(
            fetch_from_other_sources(
                &client,
                Platform::Migu,
                "60054701923|600902000006889366",
                "HQ.mp3",
            )
            .await,
            Platform::Migu,
        );
        let total = ranged_total(&client, &link.url).await;
        let delta = (total as i64 - 10_792_962).abs();
        assert!(
            delta * 1000 < 10_792_962,
            "字节数应与咪咕 HQ 档声明相差不到 0.1%，实际 {total}"
        );
        println!("咪咕 HQ.mp3 → {} 字节（{}）", total, link.quality);
    }

    /// 负面对照：全 0 hash 拿不到任何元信息，必须拒绝而不是乱给一条链接。
    #[tokio::test]
    #[ignore = "真实网络：需要酷狗与酷我接口在线"]
    async fn live_unknown_track_is_rejected() {
        let client = Client::new();

        match fetch_from_other_sources(
            &client,
            Platform::Kugou,
            "00000000000000000000000000000000|||0",
            "320.mp3",
        )
        .await
        {
            FallbackOutcome::Unavailable(reason) => {
                println!("未知曲目按预期被拒: {reason}");
            }
            FallbackOutcome::Linked(link) => panic!("未知曲目不应给出直链: {}", link.url),
            FallbackOutcome::Transient(reason) => panic!("未知曲目不应是临时错误: {reason}"),
        }
    }
}
