//! 酷狗音乐下载链接获取模块。
//!
//! 实测结论（`_dev/probe/kugou-probe.mjs` / `kugou-probe.log` / `kugou-probe-hq.log`）：
//! - **128k mp3**：`https://m.kugou.com/app/i/getSongInfo.php?cmd=playInfo&hash=<128k hash>`，
//!   无需签名，`url` 是字符串直链（host `sharefs.kugou.com`）。该接口**无视请求里的档位**：
//!   传 320k/无损 hash 仍然返回同一个 128k 文件（实测 `fileSize` 与 `req_hash` 都被回写成 128k），
//!   所以它只能用于 128k 档。
//! - **320k mp3 / 无损 flac**：`https://trackercdn.kugou.com/i/v2/`，需要
//!   `key = md5(小写 hash + "kgcloudv2")`；`br=hq` → 320k mp3，`br=flac` → 无损 flac，
//!   `url` 是**数组**（host `fsandroid.tx.kugou.com`）。
//! - `https://wwwapi.kugou.com/yy/index.php?r=play/getdata` 无签名时被风控
//!   （`status=0` / `err_code=30020`，`play_url` 恒为空），不采用。
//! - **付费/VIP 受限曲目匿名拿不到直链**（实测 `_dev/probe/kg-vip-probe.log`）：
//!   `getSongInfo` 返回 `status=0`、`privilege=10`、`pay_type=3`、`error="需要付费"`，
//!   `url` 缺失且 `fileSize=0`（`backup_url` 虽在但不可用）；`trackercdn` v2 只回 `{"status":2}`。
//!   两者统一映射为 `PAID_RESTRICTED_MESSAGE`，不回显接口原文。
//!
//! 酷狗直链不需要解密密钥，`Ok` 的第二项恒为空字符串。

use reqwest::{Client, Url};
use serde_json::Value;

use super::parser::{self, KugouMid, Quality};
use super::sign;

/// 128k mp3 直链接口（实测无需签名）。
const SONG_INFO_ENDPOINT: &str = "https://m.kugou.com/app/i/getSongInfo.php";
/// 320k mp3 / 无损 flac 直链接口（需要 `key` 签名）。
const TRACKER_ENDPOINT: &str = "https://trackercdn.kugou.com/i/v2/";

/// 探测脚本实测使用的移动端 UA（这两个接口都按移动端返回 JSON）。
const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 13; V2227A Build/TP1A.220624.014) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/114.0.0.0 Mobile Safari/537.36";
/// 探测脚本实测使用的 Referer。
const MOBILE_REFERER: &str = "https://m.kugou.com/";

/// 获取下载链接（对外统一入口）。
///
/// # 参数
/// - `client`：运行时复用的 HTTP 客户端。
/// - `song_mid`：搜索时写入的复合 `mid`（`{128k hash}|{320k hash}|{无损 hash}|{专辑 id}`）。
/// - `filename`：编码档位的文件名（`128.mp3` / `320.mp3` / `2000.flac`）。
///
/// # 返回
/// - `Ok((String, String))`：`(直链, 空密钥)`。
/// - `Err(String)`：确定性失败为中文说明；只有 `网络错误: `/`读取响应失败: `/`解析响应失败: `
///   三类前缀会被下载层视为可重试。
pub async fn get_download_link(
    client: &Client,
    song_mid: &str,
    filename: &str,
) -> Result<(String, String), String> {
    let mid = parser::parse_mid(song_mid)?;
    let quality = parser::parse_quality_filename(filename)?;
    let hash = resolve_hash(&mid, quality)?;

    let data = match quality {
        Quality::Standard => fetch_song_info(client, &hash).await?,
        Quality::Exhigh | Quality::Lossless => fetch_tracker(client, &hash, quality).await?,
    };

    ensure_success(&data)?;
    validate_container(&data, quality.extension())?;
    let url = pick_url(&data)
        .ok_or_else(|| "该曲目在酷狗无法获取直链（可能需要 VIP 或已下架）".to_string())?;

    Ok((url, String::new()))
}

/// 取该档位的 hash；档位缺失时给出确定性错误（前端只会上架存在的档位）。
fn resolve_hash(mid: &KugouMid, quality: Quality) -> Result<String, String> {
    let hash = mid.hash_for(quality);
    if hash.is_empty() {
        return Err(format!("该曲目没有 {} 音质资源", quality.label()));
    }
    Ok(hash.to_string())
}

/// 128k mp3：`getSongInfo.php`（无需签名）。
async fn fetch_song_info(client: &Client, hash: &str) -> Result<Value, String> {
    request_json(client, song_info_url(hash)?).await
}

/// 320k mp3 / 无损 flac：`trackercdn` v2（`br=hq` / `br=flac`）。
async fn fetch_tracker(client: &Client, hash: &str, quality: Quality) -> Result<Value, String> {
    request_json(client, tracker_url(hash, quality)?).await
}

/// 构造 `getSongInfo.php` 请求地址。
fn song_info_url(hash: &str) -> Result<Url, String> {
    Url::parse_with_params(SONG_INFO_ENDPOINT, &[("cmd", "playInfo"), ("hash", hash)])
        .map_err(|e| format!("URL 构建失败: {}", e))
}

/// 构造 `trackercdn` 请求地址（参数与探测脚本实测通过的一致）。
fn tracker_url(hash: &str, quality: Quality) -> Result<Url, String> {
    let key = sign::tracker_key(hash);
    let br = match quality {
        Quality::Lossless => "flac",
        _ => "hq",
    };
    Url::parse_with_params(
        TRACKER_ENDPOINT,
        &[
            ("key", key.as_str()),
            ("hash", hash),
            ("br", br),
            ("appid", "1005"),
            ("pid", "2"),
            ("cmd", "25"),
            ("behavior", "play"),
        ],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))
}

/// 发起取链请求并解析 JSON；错误分级与契约一致。
async fn request_json(client: &Client, url: Url) -> Result<Value, String> {
    let response = client
        .get(url)
        .header("User-Agent", MOBILE_USER_AGENT)
        .header("Referer", MOBILE_REFERER)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
}

/// 受限（付费/VIP）曲目的确定性失败文案。
///
/// 匿名态下这类曲目在两条取链路径上都拿不到直链，且接口给的诊断信息没有帮助
/// （`error="需要付费"`、`err_code=n/a`、`status=2`），所以统一给这句中文，
/// 既不回显接口原文，也不把 `n/a` 之类的占位符泄露到界面。
pub const PAID_RESTRICTED_MESSAGE: &str = "酷狗音乐：该歌曲为付费/VIP 曲目，匿名无法获取下载链接";

/// 状态校验：`status == 1` 才算成功。
///
/// 实测失败形态（`_dev/probe/kg-vip-probe.log`、`kg-vip-evidence.json`）：
/// - 受限曲目 `getSongInfo`：`status=0`、`privilege=10`、`pay_type=3`、`error="需要付费"`、
///   `fileSize=0`，`url` 缺失但 `backup_url` 存在（不可用 ⇒ 所以必须先校验状态再看直链）；
/// - 受限曲目 `trackercdn` v2：`{"status":2,...}`，既无 `url` 也无 `error`；
/// - 其它失败：带 `err_code`/`error` 的诊断（如 wwwapi 风控 `status=0` + `err_code=30020`）。
fn ensure_success(data: &Value) -> Result<(), String> {
    let status = data["status"].as_i64().unwrap_or_default();
    if status == 1 {
        return Ok(());
    }
    if is_paid_restricted(data, status) {
        return Err(PAID_RESTRICTED_MESSAGE.to_string());
    }
    let errcode = data["errcode"]
        .as_i64()
        .or_else(|| data["err_code"].as_i64())
        .unwrap_or_default();
    let error = data["error"].as_str().unwrap_or_default();
    Err(format!(
        "酷狗接口错误: status={}, errcode={}, error={}",
        status, errcode, error
    ))
}

/// 是否属于「付费/VIP 受限」失败。
///
/// 判据（均取自实测响应，未观察到误判样本）：
/// - `privilege` / `pay_type` 任一存在且非 0：接口自报的权限位；
/// - `error` 文案含「付费」或「VIP」；
/// - `trackercdn` 的 `status=2` 且响应里没有任何可用直链（受限 hash 实测的唯一形态）。
fn is_paid_restricted(data: &Value, status: i64) -> bool {
    if data["privilege"].as_i64().is_some_and(|value| value != 0) {
        return true;
    }
    if data["pay_type"].as_i64().is_some_and(|value| value != 0) {
        return true;
    }
    if let Some(error) = data["error"].as_str() {
        if error.contains("付费") || error.to_ascii_lowercase().contains("vip") {
            return true;
        }
    }
    status == 2 && pick_url(data).is_none()
}

/// 直链提取：`getSongInfo` 的 `url` 是字符串，`trackercdn` 的 `url` 是数组，
/// `getSongInfo` 另有 `backup_url` 数组作为兜底。
fn pick_url(data: &Value) -> Option<String> {
    for key in ["url", "urls", "backup_url"] {
        match data.get(key) {
            Some(Value::String(url)) if is_http_url(url) => return Some(url.clone()),
            Some(Value::Array(urls)) => {
                if let Some(url) = urls
                    .iter()
                    .filter_map(Value::as_str)
                    .find(|url| is_http_url(url))
                {
                    return Some(url.to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// 容器校验：落盘扩展名取自 `filename`，容器不符时宁可报错也不让扩展名说谎。
fn validate_container(data: &Value, expected: &str) -> Result<(), String> {
    let actual = data["extName"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if actual.is_empty() || actual == expected {
        return Ok(());
    }
    Err(format!(
        "酷狗返回的文件容器与音质不一致（请求 {}，实际 {}）",
        expected, actual
    ))
}

/// 只接受 http/https 直链，避免把占位字符串当成直链。
fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_success, pick_url, resolve_hash, song_info_url, tracker_url, validate_container,
        PAID_RESTRICTED_MESSAGE,
    };
    use crate::platforms::kugou::parser::Quality;
    use serde_json::{json, Value};

    /// 实测样本的复合 mid（蓝心羽 - 晴天）。
    fn parse_mid_fixture() -> crate::platforms::kugou::parser::KugouMid {
        crate::platforms::kugou::parser::parse_mid(
            "48c685f679ffc7cf08b8a8341ca9db44|28f83bbd8cab043895039e98366ad2b4|857879479e698d729640a5716dc3e56f|79360569",
        )
        .expect("样本 mid 应能解析")
    }

    #[test]
    fn tracker_url_carries_probed_signature_and_params() {
        let url = tracker_url("28f83bbd8cab043895039e98366ad2b4", Quality::Exhigh).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("trackercdn.kugou.com"));
        assert_eq!(url.path(), "/i/v2/");

        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        for expected in [
            // 实测：这个 key 真的换回了 320k 直链
            ("key", "0d8fc4aa46dc995ef3e43cb260b81145"),
            ("hash", "28f83bbd8cab043895039e98366ad2b4"),
            ("br", "hq"),
            ("appid", "1005"),
            ("pid", "2"),
            ("cmd", "25"),
            ("behavior", "play"),
        ] {
            assert!(
                pairs
                    .iter()
                    .any(|(key, value)| key == expected.0 && value == expected.1),
                "缺少参数 {}={}：{:?}",
                expected.0,
                expected.1,
                pairs
            );
        }

        let flac = tracker_url("857879479e698d729640a5716dc3e56f", Quality::Lossless).unwrap();
        let flac_pairs: Vec<(String, String)> = flac
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        assert!(flac_pairs
            .iter()
            .any(|(key, value)| key == "key" && value == "ced4966efac362219f9d884dfe8303ec"));
        assert!(flac_pairs
            .iter()
            .any(|(key, value)| key == "br" && value == "flac"));
    }

    #[test]
    fn song_info_url_matches_probed_request() {
        let url = song_info_url("48c685f679ffc7cf08b8a8341ca9db44").unwrap();
        assert_eq!(url.host_str(), Some("m.kugou.com"));
        assert_eq!(url.path(), "/app/i/getSongInfo.php");
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        assert!(pairs
            .iter()
            .any(|(key, value)| key == "cmd" && value == "playInfo"));
        assert!(pairs
            .iter()
            .any(|(key, value)| key == "hash" && value == "48c685f679ffc7cf08b8a8341ca9db44"));
    }

    #[test]
    fn resolve_hash_reports_missing_tiers() {
        let mid = parse_mid_fixture();
        assert_eq!(
            resolve_hash(&mid, Quality::Standard).unwrap(),
            "48c685f679ffc7cf08b8a8341ca9db44"
        );
        assert_eq!(
            resolve_hash(&mid, Quality::Exhigh).unwrap(),
            "28f83bbd8cab043895039e98366ad2b4"
        );
        assert_eq!(
            resolve_hash(&mid, Quality::Lossless).unwrap(),
            "857879479e698d729640a5716dc3e56f"
        );

        // 裸 hash：只有 128k 档，缺档位要报确定性错误
        let bare =
            crate::platforms::kugou::parser::parse_mid("48c685f679ffc7cf08b8a8341ca9db44").unwrap();
        let exhigh_error = resolve_hash(&bare, Quality::Exhigh).unwrap_err();
        assert!(exhigh_error.contains("320kmp3"), "{}", exhigh_error);
        let lossless_error = resolve_hash(&bare, Quality::Lossless).unwrap_err();
        assert!(lossless_error.contains("flac"), "{}", lossless_error);
    }

    #[test]
    fn pick_url_reads_string_and_array_shapes() {
        // getSongInfo：url 是字符串
        let string_shape = json!({ "url": "http://sharefs.kugou.com/a.mp3" });
        assert_eq!(
            pick_url(&string_shape).as_deref(),
            Some("http://sharefs.kugou.com/a.mp3")
        );
        // trackercdn：url 是数组
        let array_shape =
            json!({ "url": ["https://fsandroid.tx.kugou.com/a.flac", "https://x/b.flac"] });
        assert_eq!(
            pick_url(&array_shape).as_deref(),
            Some("https://fsandroid.tx.kugou.com/a.flac")
        );
        // 兜底：backup_url
        let backup_shape = json!({ "backup_url": ["https://sharefs.tx.kugou.com/a.mp3"] });
        assert_eq!(
            pick_url(&backup_shape).as_deref(),
            Some("https://sharefs.tx.kugou.com/a.mp3")
        );
        // 空/占位值不算直链
        assert!(pick_url(&json!({ "url": "" })).is_none());
        assert!(pick_url(&json!({ "url": ["", "not-a-url"] })).is_none());
        assert!(pick_url(&json!({})).is_none());
    }

    #[test]
    fn ensure_success_matches_probed_shapes() {
        // 实测成功形态
        assert!(ensure_success(&json!({ "status": 1, "errcode": 0 })).is_ok());
        // 实测失败形态（wwwapi 风控）
        let error = ensure_success(&json!({ "status": 0, "err_code": 30020 })).unwrap_err();
        assert!(error.contains("status=0"));
        assert!(error.contains("30020"));
        // 缺 status 时按失败处理
        assert!(ensure_success(&json!({ "error": "bad key" })).is_err());
    }

    #[test]
    fn container_mismatch_is_rejected() {
        assert!(validate_container(&json!({ "extName": "mp3" }), "mp3").is_ok());
        assert!(validate_container(&json!({ "extName": ".flac" }), "flac").is_ok());
        // 接口没给容器信息时不拦（静态推断：容器由请求档位决定）
        assert!(validate_container(&json!({}), "mp3").is_ok());
        let error = validate_container(&json!({ "extName": "flac" }), "mp3").unwrap_err();
        assert!(error.contains("容器与音质不一致"));
    }

    /// task-38 实测样本：周杰伦 原版《晴天》（`privilege=10 pay_type=3`）的 `getSongInfo` 响应
    /// （见 `_dev/probe/kg-vip-evidence.json` 的 VIP-A，这里只保留判据相关字段）。
    fn restricted_song_info_fixture() -> Value {
        json!({
            "hash": "B3A52A7A958BF0AED0EBFBA2E9A818B7",
            "status": 0,
            "errcode": 0,
            "error": "需要付费",
            "privilege": 10,
            "pay_type": 3,
            "fail_process": 12,
            "fileSize": 0,
            "bitRate": 0,
            "timeLength": 0,
            "fileName": "周杰伦 - 晴天",
            "backup_url": ["http://fsandroid.tx.kugou.com/202610031720/example.mp3"],
        })
    }

    #[test]
    fn restricted_song_info_maps_to_paid_message() {
        let data = restricted_song_info_fixture();
        // 先固定「为什么必须先校验状态」：受限曲目的 backup_url 是存在且可解析的
        assert!(pick_url(&data).is_some());
        let error = ensure_success(&data).unwrap_err();
        assert_eq!(error, PAID_RESTRICTED_MESSAGE);
        // 不得回显接口原文、字段名或占位符
        for leaked in [
            "需要付费",
            "status=",
            "errcode=",
            "n/a",
            "backup_url",
            "fail_process",
        ] {
            assert!(!error.contains(leaked), "错误文案泄露了 {leaked}：{error}");
        }
    }

    #[test]
    fn restricted_tracker_status_two_maps_to_paid_message() {
        // 实测 VIP-B：trackercdn v2 对受限 320hash 的完整响应（89 字节，除 status 外只有 trans_param）
        let data = json!({
            "status": 2,
            "trans_param": {
                "classmap": { "attr0": 234885128 },
                "display": 32,
                "display_rate": 1,
            },
        });
        assert_eq!(ensure_success(&data).unwrap_err(), PAID_RESTRICTED_MESSAGE);
    }

    #[test]
    fn paid_flags_alone_are_enough_to_report_paid_message() {
        // 只有权限位、没有 error 文案时也要能认出来（保守推断：搜索接口自报 privilege 非 0）
        assert_eq!(
            ensure_success(&json!({ "status": 0, "privilege": 8 })).unwrap_err(),
            PAID_RESTRICTED_MESSAGE
        );
        assert_eq!(
            ensure_success(&json!({ "status": 0, "pay_type": 3 })).unwrap_err(),
            PAID_RESTRICTED_MESSAGE
        );
        assert_eq!(
            ensure_success(&json!({ "status": 0, "error": "该歌曲需要VIP" })).unwrap_err(),
            PAID_RESTRICTED_MESSAGE
        );
    }

    #[test]
    fn unrelated_failure_keeps_diagnostic_and_is_not_paid() {
        let error =
            ensure_success(&json!({ "status": 0, "err_code": 20001, "error": "hash 不存在" }))
                .unwrap_err();
        assert!(error.contains("status=0"));
        assert!(error.contains("20001"));
        assert!(error.contains("hash 不存在"));
        assert_ne!(error, PAID_RESTRICTED_MESSAGE);
        // status=2 但拿到了直链 ⇒ 不是受限（避免把「有 url 的 status=2」误判成付费）
        assert!(ensure_success(&json!({
            "status": 2,
            "url": "http://fsandroid.tx.kugou.com/example.mp3",
        }))
        .is_err());
        assert_ne!(
            ensure_success(&json!({
                "status": 2,
                "url": "http://fsandroid.tx.kugou.com/example.mp3",
            }))
            .unwrap_err(),
            PAID_RESTRICTED_MESSAGE
        );
    }
}
