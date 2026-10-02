//! 歌曲搜索模块。
//!
//! 通过 QQ 音乐桌面端搜索接口（`music.search.SearchCgiService.DoSearchForQQMusicDesktop`）
//! 搜索歌曲、歌手与专辑，返回解析后的列表和分页信息。
//! 旧的移动端方法 `DoSearchForQQMusicLite` 已失效：接口仍返回 `code=0`，
//! 但 `item_song` / `singer` / `item_album` 恒为空数组，前端因此只会看到「暂无搜索结果」。
//!
//! 请求优先走带 `zzc` 签名的 `musics.fcg`：实测未签名的 `musicu.fcg` 会被限流
//! （子请求 `code=2001`），未签名的 `musics.fcg` 直接报 `code=2000` 要求签名，
//! 只有签名请求在同一下行时刻仍能正常返回结果，签名算法见 [`super::sign`]。
//! 歌曲解析复用 [`super::parser::parse_song`] 函数。
//!
//! 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/tx/musicSearch.js#L13>

use std::time::Duration;

use serde_json::{json, Value};

use super::parser::parse_song;
use super::sign::zzc_sign;
use crate::platforms::{get_guid, CLIENT};

/// 带签名的接口地址：`?sign=<zzc 签名>` 由请求体算出，实测能绕过未签名接口的限流。
const SIGNED_SEARCH_ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musics.fcg";

/// 未签名的接口地址，仅在签名请求失败时兜底。
const SEARCH_ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";

/// 桌面端搜索的模块名与方法名，同时作为请求体的顶层键与响应中的子响应键。
const SEARCH_MODULE: &str = "music.search.SearchCgiService";
const SEARCH_METHOD: &str = "DoSearchForQQMusicDesktop";

/// 桌面端客户端的设备标识（`comm.wid`），与 `guid` 一起构成匿名客户端指纹。
const PC_WID: &str = "7223299733393904640";

/// 接口偶发风控（子请求 `code=2001`，实测同一 IP 约每 3 次出现 1 次），
/// 最多尝试次数与递增退避间隔：300ms / 600ms / 1200ms，最坏约 2.1s 返回。
const MAX_ATTEMPTS: u32 = 4;
const RETRY_BACKOFF_MS: u64 = 300;

/// 搜索歌曲，返回 JSON 数组字符串（扩展 SongInfo，增加 `mediaMid` 和 `qualities`）。
///
/// 该命令调用 QQ 音乐桌面端搜索接口，根据关键字、页码和每页数量搜索歌曲。
/// 请求中需要动态生成 `searchid` 和 `guid`，并携带设备信息等参数以模拟真实客户端。
/// 搜索结果中的每首歌曲通过 [`parse_song`] 解析，并返回分页标志 `has_more`。
///
/// # 参数
/// - `keyword`: 搜索关键字。
/// - `page`: 页码（从 1 开始）。
/// - `limit`: 每页歌曲数量。
///
/// # 返回
/// - `Ok(String)`：JSON 字符串，包含 `songs`（歌曲数组）和 `has_more`（是否有下一页）两个字段。
/// - `Err(String)`：错误信息，包括网络错误、接口错误、序列化错误等。
pub async fn search_songs(
    separator: &str,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = search_request(keyword, page, limit, 0).await?;

    // 提取歌曲列表
    let item_song = data["body"]["song"]["list"]
        .as_array()
        .ok_or("未找到歌曲列表")?;

    // 分页判断：优先按接口返回的总数 meta.sum 推算，避免因 parse_song 过滤导致
    // 有效歌曲数不足一页时，误判为无更多结果，影响「加载更多」按钮显示。
    let has_more = has_more(&data["meta"], page, limit);

    // 逐首解析歌曲，过滤无法解析的条目
    let mut songs: Vec<Value> = Vec::new();
    for item in item_song {
        if let Some(song_obj) = parse_song(item, separator) {
            songs.push(song_obj);
        }
    }

    // 返回包含歌曲列表和分页标志的 JSON 对象
    let result = json!({
        "songs": songs,
        "has_more": has_more
    });

    serde_json::to_string(&result).map_err(|e| format!("序列化结果失败: {}", e))
}

/// 共用平台搜索请求，搜索类型决定响应列表字段。
///
/// 接口对同一 IP 存在偶发风控（子请求 `code=2001`）：实测同一关键词每 3 次约有 1 次失败，
/// 失败请求不重试就会让前端看到「暂无搜索结果」。这里对**错误**最多尝试 [`MAX_ATTEMPTS`] 次，
/// 退避间隔逐次翻倍；`code=0` 的结果即便为空也直接返回（那是真实的「没有结果」，
/// 重试既无意义又会加重风控）。全部失败时返回最后一次的错误，由前端提示用户重试。
async fn search_request(
    keyword: String,
    page: u32,
    limit: u32,
    search_type: u32,
) -> Result<Value, String> {
    let mut last_error = String::from("搜索请求失败");

    for attempt in 1..=MAX_ATTEMPTS {
        match search_request_once(&keyword, page, limit, search_type).await {
            Ok(data) => return Ok(data),
            Err(error) => last_error = error,
        }

        if attempt < MAX_ATTEMPTS {
            let backoff = RETRY_BACKOFF_MS * (1 << (attempt - 1));
            tokio::time::sleep(Duration::from_millis(backoff)).await;
        }
    }

    Err(last_error)
}

/// 单次搜索请求，返回子响应的 `data` 字段。
///
/// 先带签名请求 `musics.fcg`，失败再退回未签名的 `musicu.fcg`；
/// 两次都失败时返回后者的错误信息。
async fn search_request_once(
    keyword: &str,
    page: u32,
    limit: u32,
    search_type: u32,
) -> Result<Value, String> {
    // 构造请求体：桌面端把子请求直接挂在模块名键下，不再使用 `req` 包装。
    let request_body = json!({
        "comm": pc_comm(),
        SEARCH_MODULE: {
            "module": SEARCH_MODULE,
            "method": SEARCH_METHOD,
            "param": {
                "search_type": search_type,
                // 0 是搜索歌曲，1 是搜索歌手，2 是搜索专辑，不同 type 返回的列表字段不同
                "searchid": new_search_id(),
                "query": keyword,
                "page_num": page,
                "num_per_page": limit,
                "remoteplace": "txt.newclient.top",
                "grp": 1
            }
        }
    });
    let body_text =
        serde_json::to_string(&request_body).map_err(|e| format!("请求体序列化失败: {}", e))?;

    // 签名必须覆盖真正发出去的字节，所以先序列化再签名。
    let signed_url = format!("{}?sign={}", SIGNED_SEARCH_ENDPOINT, zzc_sign(&body_text));

    let mut last_error = String::from("搜索请求失败");
    for url in [signed_url.as_str(), SEARCH_ENDPOINT] {
        match post_search(url, &body_text).await {
            Ok(data) => return Ok(data),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

/// 发送一次搜索请求并校验业务状态码。
async fn post_search(url: &str, body_text: &str) -> Result<Value, String> {
    // 发送 POST 请求
    let resp = CLIENT
        .post(url)
        .header("Content-Type", "application/json")
        .header("Referer", "https://y.qq.com")
        .body(body_text.to_string())
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 检查整体状态与子请求状态
    if data["code"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!(
            "QQ 音乐接口返回异常（code={}），请稍后重试",
            data["code"]
        ));
    }
    let module = &data[SEARCH_MODULE];
    let module_code = module["code"].as_i64().unwrap_or(-1);
    if module_code != 0 {
        // 2001 是风控/限流的固定错误码，文案要与「没有结果」区分开
        return Err(if module_code == 2001 {
            "QQ 音乐搜索暂时被限流，请稍后重试".to_string()
        } else {
            format!("搜索失败（QQ 音乐返回 {}），请稍后重试", module_code)
        });
    }

    let payload = module["data"].clone();
    if payload.is_null() {
        return Err("搜索响应缺少 data 字段".to_string());
    }
    Ok(payload)
}

/// 生成 32 位十六进制大写 + 5 位数字的搜索 ID，形状与桌面端客户端一致。
fn new_search_id() -> String {
    use rand::Rng;
    const HEX: &[u8] = b"0123456789ABCDEF";

    let mut rng = rand::rng();
    let mut searchid = String::with_capacity(37);
    for _ in 0..32 {
        searchid.push(HEX[rng.random_range(0..HEX.len())] as char);
    }
    searchid.push_str(&format!("{:05}", rng.random_range(0..100_000u32)));
    searchid
}

/// 分页判断：优先用总数 `meta.sum` 推算，缺失时回退到 `meta.nextpage`。
fn has_more(meta: &Value, page: u32, limit: u32) -> bool {
    let sum = meta["sum"].as_i64().unwrap_or(0);
    if sum > 0 {
        return sum > (page as i64) * (limit as i64);
    }
    meta["nextpage"].as_i64().unwrap_or(-1) > page as i64
}

/// 桌面端公共参数（`comm`）。
fn pc_comm() -> Value {
    json!({
        "_channelid": "0",
        "_os_version": "6.2.9200-2",
        "ct": "19",
        "cv": "2151",
        "guid": get_guid(),
        "patch": "118",
        "psrf_access_token_expiresAt": 0,
        "psrf_qqaccess_token": "",
        "psrf_qqopenid": "",
        "psrf_qqunionid": "",
        "tmeAppID": "qqmusic",
        "tmeLoginType": 0,
        "uin": "0",
        "wid": PC_WID
    })
}

/// 搜索歌手，共用搜索请求。
pub async fn search_artists(keyword: String, page: u32, limit: u32) -> Result<String, String> {
    let data = search_request(keyword, page, limit, 1).await?;
    let items = data["body"]["singer"]["list"]
        .as_array()
        .ok_or("未找到歌手列表")?;
    let artists: Vec<Value> = items
        .iter()
        .filter_map(super::artist::parse_artist)
        .collect();
    Ok(json!({
        "artists": artists,
        "has_more": has_more(&data["meta"], page, limit)
    })
    .to_string())
}

/// 搜索专辑，共用搜索请求。
pub async fn search_albums(
    separator: &str,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let data = search_request(keyword, page, limit, 2).await?;
    let items = data["body"]["album"]["list"]
        .as_array()
        .ok_or("未找到专辑列表")?;

    let albums: Vec<Value> = items
        .iter()
        .filter_map(|item| {
            // 桌面端字段为 albumMID/albumName/albumPic/publicTime，保留旧移动端字段回退
            let mid = item["albumMID"]
                .as_str()
                .or_else(|| item["albummid"].as_str())
                .filter(|s| !s.is_empty())?;

            let artists = super::parser::parse_artists(&item["singer_list"]);
            let names: Vec<&str> = artists
                .iter()
                .filter_map(|artist| artist["name"].as_str())
                .collect();
            let artist = if names.is_empty() {
                item["singerName"]
                    .as_str()
                    .or_else(|| item["singer_name"].as_str())
                    .unwrap_or("")
                    .to_owned()
            } else {
                names.join(separator)
            };

            let cover = item["albumPic"]
                .as_str()
                .or_else(|| item["pic"].as_str())
                .unwrap_or("")
                .replace("http://", "https://");
            let cover_url = if cover.is_empty() {
                format!(
                    "https://y.gtimg.cn/music/photo_new/T002R300x300M000{}.jpg",
                    mid
                )
            } else {
                cover
            };

            Some(json!({
                "id": mid,
                "name": item["albumName"]
                    .as_str()
                    .or_else(|| item["name"].as_str())
                    .unwrap_or(""),
                "artist": artist,
                "artists": artists,
                "coverUrl": cover_url,
                "publishDate": item["publicTime"]
                    .as_str()
                    .or_else(|| item["description"].as_str())
                    .unwrap_or(""),
                "songCount": item["song_count"]
                    .as_u64()
                    .or_else(|| item["song_num"].as_u64())
                    .or_else(|| {
                        item["song_count"]
                            .as_str()
                            .and_then(|s| s.parse().ok())
                            .or_else(|| item["song_num"].as_str().and_then(|s| s.parse().ok()))
                    })
                    .unwrap_or(0)
            }))
        })
        .collect();

    Ok(json!({
        "albums": albums,
        "has_more": has_more(&data["meta"], page, limit)
    })
    .to_string())
}

/// 移动端公共参数（`comm`）。
///
/// 桌面端搜索已改用 [`pc_comm`]；该函数供仍在使用移动端请求格式的模块
/// （如歌手详情）复用。
pub(super) fn mobile_comm() -> Value {
    json!({
        "ct": "11",
        "cv": "14090508",
        "v": "14090508",
        "tmeAppID": "qqmusic",
        "guid": get_guid(),
        "phonetype": "EBG-AN10",
        "deviceScore": "553.47",
        "devicelevel": "50",
        "newdevicelevel": "20",
        "rom": "HuaWei/EMOTION/EmotionUI_14.2.0",
        "os_ver": "12",
        "OpenUDID": "0",
        "OpenUDID2": "0",
        "QIMEI36": "0",
        "udid": "0",
        "chid": "0",
        "aid": "0",
        "oaid": "0",
        "taid": "0",
        "tid": "0",
        "wid": "0",
        "uid": "0",
        "sid": "0",
        "modeSwitch": "6",
        "teenMode": "0",
        "ui_mode": "2",
        "nettype": "1020",
        "v4ip": ""
    })
}
