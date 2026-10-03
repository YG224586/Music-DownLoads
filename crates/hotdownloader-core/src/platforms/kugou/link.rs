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
//! - **登录态**：`https://wwwapi.kugou.com/yy/index.php?r=play/getdata` 只有带
//!   `token` + `userid` 的 Cookie 才能拿到直链；匿名（含随机设备字段）恒回 `err_code=30020`
//!   「需要登录」，因此这条路径只在配置了酷狗账号时尝试。
//! - **付费/VIP 受限曲目匿名拿不到直链**（实测 `_dev/probe/kg-vip-probe.log`）：
//!   `getSongInfo` 返回 `status=0`、`privilege=10`、`pay_type=3`、`error="需要付费"`，
//!   `url` 缺失且 `fileSize=0`（`backup_url` 虽在但不可用）；`trackercdn` v2 只回 `{"status":2}`。
//!
//! 取链顺序：配置了酷狗账号时**先**走登录态请求（由酷狗按该账号权益签发直链），
//! 登录态没拿到完整直链时再按酷狗自身的匿名能力取链；两条路都没成，则按「是否配置了
//! 可用账号」给出两种确定性中文文案。整条链路只使用酷狗自己的接口，不存在把这首曲子
//! 交给别的平台的代码路径。
//!
//! 酷狗直链不需要解密密钥，`Ok` 的第二项恒为空字符串。

use reqwest::{Client, Url};
use serde_json::Value;

use super::credentials::KugouCookie;
use super::parser::{self, KugouMid, Quality};
use super::sign;

/// 128k mp3 直链接口（实测无需签名）。
const SONG_INFO_ENDPOINT: &str = "https://m.kugou.com/app/i/getSongInfo.php";
/// 320k mp3 / 无损 flac 直链接口（需要 `key` 签名）。
const TRACKER_ENDPOINT: &str = "https://trackercdn.kugou.com/i/v2/";
/// 登录态直链接口：由酷狗按当前账号的权益签发直链（需要 `token` + `userid`）。
const LOGIN_PLAY_ENDPOINT: &str = "https://wwwapi.kugou.com/yy/index.php";

/// 探测脚本实测使用的移动端 UA（这几个接口都按移动端返回 JSON）。
const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 13; V2227A Build/TP1A.220624.014) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/114.0.0.0 Mobile Safari/537.36";
/// 探测脚本实测使用的 Referer。
const MOBILE_REFERER: &str = "https://m.kugou.com/";

/// 未配置账号（或 Cookie 里没有 `token`+`userid`）时的确定性失败文案。
pub const PAID_RESTRICTED_MESSAGE: &str =
    "酷狗音乐：该歌曲为付费/VIP 曲目，需要酷狗会员账号，请在「设置 → 平台账号」填入酷狗 Cookie 后重试";

/// 已配置账号却没有该曲目权益时的确定性失败文案。
pub const ACCOUNT_NO_PERMISSION_MESSAGE: &str =
    "酷狗音乐：当前酷狗账号没有该曲目的下载权限（付费/会员曲目），请确认账号已开通权益且 Cookie 未过期";

/// 获取下载链接（对外统一入口）。
///
/// # 参数
/// - `client`：运行时复用的 HTTP 客户端。
/// - `song_mid`：搜索时写入的复合 `mid`（`{128k hash}|{320k hash}|{无损 hash}|{专辑 id}`）。
/// - `filename`：编码档位的文件名（`128.mp3` / `320.mp3` / `2000.flac`）。
/// - `cookie`：本机保存的酷狗账号 Cookie；`None` / 空白串 = 按匿名能力取链。
///
/// # 返回
/// - `Ok((String, String))`：`(直链, 空密钥)`。
/// - `Err(String)`：确定性失败为中文说明；只有 `网络错误: `/`读取响应失败: `/`解析响应失败: `
///   三类前缀会被下载层视为可重试。
pub async fn get_download_link(
    client: &Client,
    song_mid: &str,
    filename: &str,
    cookie: Option<&str>,
) -> Result<(String, String), String> {
    let mid = parser::parse_mid(song_mid)?;
    let quality = parser::parse_quality_filename(filename)?;
    let hash = resolve_hash(&mid, quality)?;

    // 只有拿到 token + userid 才算「配置了账号」：只填设备字段的 Cookie 去请求登录态
    // 也只是白跑一次（酷狗照样回 err_code=30020）。
    let account = logged_in_account(cookie);

    // 1) 登录态：请酷狗按该账号的权益签发直链，成功即返回。
    if let Some(account) = account.as_ref() {
        if let Some(url) = fetch_login_link(client, &hash, &mid, quality, account).await {
            return Ok((url, String::new()));
        }
    }

    // 2) 登录态没拿到（或没配置账号）：按酷狗自身的匿名能力取链。
    let data = match quality {
        Quality::Standard => fetch_song_info(client, &hash).await?,
        Quality::Exhigh | Quality::Lossless => fetch_tracker(client, &hash, quality).await?,
    };

    if let Err(failure) = ensure_success(&data) {
        return Err(failure_message(failure, account.as_ref()));
    }
    validate_container(&data, quality.extension())?;
    let url = pick_url(&data).ok_or_else(|| {
        // `status=1` 却没有直链：按受限处理，让用户拿到「填账号 / 查权益」这种可执行的结论，
        // 而不是一句没有下一步的「无法获取直链」。
        failure_message(LinkFailure::Restricted, account.as_ref())
    })?;

    Ok((url, String::new()))
}

/// 取出**能发起登录态请求**的账号：Cookie 非空白且含 `token` + `userid`。
///
/// 只有设备字段（`dfid`/`mid`）的 Cookie 功能上等同于没配置账号，因此按匿名处理，
/// 最终文案也会提示用户补一份完整 Cookie。
fn logged_in_account(cookie: Option<&str>) -> Option<KugouCookie> {
    let raw = cookie.map(str::trim).filter(|raw| !raw.is_empty())?;
    let account = KugouCookie::parse(raw);
    account.is_logged_in().then_some(account)
}

/// 取该档位的 hash；档位缺失时给出确定性错误（前端只会上架存在的档位）。
fn resolve_hash(mid: &KugouMid, quality: Quality) -> Result<String, String> {
    let hash = mid.hash_for(quality);
    if hash.is_empty() {
        return Err(format!("该曲目没有 {} 音质资源", quality.label()));
    }
    Ok(hash.to_string())
}

/// 登录态取链：请酷狗按该账号的权益签发直链。
///
/// 返回 `None` 表示这条路径没拿到**完整**直链（请求失败、Cookie 失效、曲目不在该账号
/// 权益内、只给试听片段、容器与档位不符），调用方据此继续按匿名能力取链。
/// 这里不记录请求 URL（它带 `token`/`userid`），也不记录 Cookie。
async fn fetch_login_link(
    client: &Client,
    hash: &str,
    mid: &KugouMid,
    quality: Quality,
    account: &KugouCookie,
) -> Option<String> {
    let url = login_play_url(hash, mid, account).ok()?;
    let data = request_login_json(client, url, account.raw()).await?;
    let payload = data.get("data")?;
    if !payload.is_object() {
        return None;
    }
    if validate_container(payload, quality.extension()).is_err() {
        log::debug!("酷狗登录态返回的容器与请求档位不一致，改按匿名能力取链");
        return None;
    }

    let picked = pick_login_url(payload);
    if picked.is_none() {
        log::debug!("酷狗登录态未返回完整直链（账号无该曲目权益，或只返回试听片段）");
    }
    picked
}

/// 构造登录态取链地址（`r=play/getdata` + 账号四要素，参数与实测一致）。
///
/// `dfid`/`mid` 缺失时留空：酷狗会退回设备匿名判定，那一侧本来就拿不到付费曲。
fn login_play_url(hash: &str, mid: &KugouMid, account: &KugouCookie) -> Result<Url, String> {
    Url::parse_with_params(
        LOGIN_PLAY_ENDPOINT,
        &[
            ("r", "play/getdata"),
            ("hash", hash),
            ("album_id", mid.album_id.as_str()),
            ("dfid", account.dfid()),
            ("mid", account.mid()),
            ("platid", "4"),
            ("appid", "1014"),
            ("clientver", "20000"),
            ("token", account.token()),
            ("userid", account.userid()),
        ],
    )
    .map_err(|e| format!("URL 构建失败: {}", e))
}

/// 发送登录态请求并解析 JSON；任何一步失败都返回 `None`。
///
/// 登录态是「先试一次」的路径：失败不影响随后按匿名能力取链，因此不需要把错误分级
/// 上抛。Cookie 只作为请求头发送，不写日志、不进入错误文案。
async fn request_login_json(client: &Client, url: Url, raw_cookie: &str) -> Option<Value> {
    let response = client
        .get(url)
        .header("User-Agent", MOBILE_USER_AGENT)
        .header("Referer", MOBILE_REFERER)
        .header("Accept", "application/json")
        .header("Cookie", raw_cookie)
        .send()
        .await
        .ok()?;
    let text = response.text().await.ok()?;
    serde_json::from_str(&text).ok()
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

/// 发起匿名取链请求并解析 JSON；错误分级与契约一致。
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

/// 接口级失败分类。
#[derive(Debug)]
enum LinkFailure {
    /// 付费/VIP 受限（含接口自报需要权益、`status=1` 却没有直链）。
    Restricted,
    /// 其它确定性失败：保留接口诊断便于排查。
    Diagnostic(String),
}

/// 失败文案：受限类按「是否配置了可用账号」二分。
///
/// 同一个「拿不到直链」在两种状态下需要完全不同的下一步动作，所以文案必须分开：
/// 没配账号 → 去设置页填 Cookie；配了账号 → 查权益是否开通、Cookie 是否过期。
fn failure_message(failure: LinkFailure, account: Option<&KugouCookie>) -> String {
    match failure {
        LinkFailure::Restricted if account.is_some_and(KugouCookie::is_logged_in) => {
            ACCOUNT_NO_PERMISSION_MESSAGE.to_string()
        }
        LinkFailure::Restricted => PAID_RESTRICTED_MESSAGE.to_string(),
        LinkFailure::Diagnostic(message) => message,
    }
}

/// 状态校验：`status == 1` 才算成功。
///
/// 实测失败形态（`_dev/probe/kg-vip-probe.log`、`kg-vip-evidence.json`）：
/// - 受限曲目 `getSongInfo`：`status=0`、`privilege=10`、`pay_type=3`、`error="需要付费"`、
///   `fileSize=0`，`url` 缺失但 `backup_url` 存在（不可用 ⇒ 所以必须先校验状态再看直链）；
/// - 受限曲目 `trackercdn` v2：`{"status":2,...}`，既无 `url` 也无 `error`；
/// - 其它失败：带 `err_code`/`error` 的诊断（如 wwwapi 风控 `status=0` + `err_code=30020`）。
fn ensure_success(data: &Value) -> Result<(), LinkFailure> {
    let status = data["status"].as_i64().unwrap_or_default();
    if status == 1 {
        return Ok(());
    }
    if is_paid_restricted(data, status) {
        return Err(LinkFailure::Restricted);
    }
    let errcode = data["errcode"]
        .as_i64()
        .or_else(|| data["err_code"].as_i64())
        .unwrap_or_default();
    let error = data["error"].as_str().unwrap_or_default();
    Err(LinkFailure::Diagnostic(format!(
        "酷狗接口错误: status={}, errcode={}, error={}",
        status, errcode, error
    )))
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
    pick_by_keys(data, &["url", "urls", "backup_url"])
}

/// 登录态直链提取：`data.play_url` → `data.play_backup_url`。
///
/// `is_free_part == 1` 或直链里带 `trial` 都说明这是**试听片段**：片段不能被当成完整歌曲
/// 交付，因此按「没拿到直链」处理（返回 `None`，由匿名链或最终文案给结论）。
fn pick_login_url(payload: &Value) -> Option<String> {
    if payload.get("is_free_part").and_then(Value::as_i64) == Some(1) {
        return None;
    }
    for key in ["play_url", "play_backup_url"] {
        if let Some(url) = pick_by_keys(payload, &[key]) {
            if !url.to_ascii_lowercase().contains("trial") {
                return Some(url);
            }
        }
    }
    None
}

/// 按给定键顺序取第一个可用直链（字符串或数组两种形状都支持）。
fn pick_by_keys(data: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        match data.get(*key) {
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
        ensure_success, failure_message, logged_in_account, login_play_url, pick_login_url,
        pick_url, resolve_hash, song_info_url, tracker_url, validate_container, LinkFailure,
        ACCOUNT_NO_PERMISSION_MESSAGE, PAID_RESTRICTED_MESSAGE,
    };
    use crate::platforms::kugou::credentials::KugouCookie;
    use crate::platforms::kugou::parser::Quality;
    use serde_json::{json, Value};

    /// 实测样本的复合 mid（蓝心羽 - 晴天）。
    fn parse_mid_fixture() -> crate::platforms::kugou::parser::KugouMid {
        crate::platforms::kugou::parser::parse_mid(
            "48c685f679ffc7cf08b8a8341ca9db44|28f83bbd8cab043895039e98366ad2b4|857879479e698d729640a5716dc3e56f|79360569",
        )
        .expect("样本 mid 应能解析")
    }

    /// 一份「能发起登录态请求」的浏览器 Cookie。
    fn logged_in_cookie() -> KugouCookie {
        KugouCookie::parse("kg_mid=mid-abc; kg_dfid=dfid-99; token=tok-7; userid=1234567")
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
    fn login_play_url_carries_account_and_album_fields() {
        let mid = parse_mid_fixture();
        let url = login_play_url(
            "28f83bbd8cab043895039e98366ad2b4",
            &mid,
            &logged_in_cookie(),
        )
        .expect("登录态 URL 应能构造");

        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("wwwapi.kugou.com"));
        assert_eq!(url.path(), "/yy/index.php");

        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        for expected in [
            ("r", "play/getdata"),
            ("hash", "28f83bbd8cab043895039e98366ad2b4"),
            // album_id 来自复合 mid 的第 4 段（见 kugou/parser.rs 的 KugouMid::album_id）
            ("album_id", "79360569"),
            ("dfid", "dfid-99"),
            ("mid", "mid-abc"),
            ("platid", "4"),
            ("appid", "1014"),
            ("clientver", "20000"),
            ("token", "tok-7"),
            ("userid", "1234567"),
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
    }

    #[test]
    fn login_path_requires_token_and_userid() {
        // 没配置 / 只填空白 = 匿名
        assert!(logged_in_account(None).is_none());
        assert!(logged_in_account(Some("   ")).is_none());
        // 只有设备字段：功能上等同于没配置账号（这条 Cookie 请求登录态也只会拿到 30020）
        assert!(logged_in_account(Some("kg_mid=mid-abc; kg_dfid=dfid-99")).is_none());
        assert!(logged_in_account(Some("token=tok-7")).is_none());
        // 账号四要素齐全才走登录态
        let account = logged_in_account(Some(" kg_mid=mid-abc; token=tok-7; userid=1234567 "))
            .expect("含 token + userid 的 Cookie 应能走登录态");
        assert_eq!(account.token(), "tok-7");
        assert_eq!(account.userid(), "1234567");
        assert_eq!(account.raw(), "kg_mid=mid-abc; token=tok-7; userid=1234567");
    }

    #[test]
    fn pick_login_url_reads_account_payload_shapes() {
        // 实测成功形态：data.play_url 是字符串，play_backup_url 是数组
        let full = json!({
            "is_free_part": 0,
            "play_url": "https://fsandroid.tx.kugou.com/20261004/a.mp3?token=x",
            "play_backup_url": ["https://sharefs.tx.kugou.com/b.mp3"],
            "fileSize": 9526125,
            "extName": "mp3",
        });
        assert_eq!(
            pick_login_url(&full).as_deref(),
            Some("https://fsandroid.tx.kugou.com/20261004/a.mp3?token=x")
        );

        // play_url 缺失时用 play_backup_url 兜底（字符串/数组两种形状都认）
        assert_eq!(
            pick_login_url(&json!({ "play_backup_url": ["https://sharefs.tx.kugou.com/b.mp3"] }))
                .as_deref(),
            Some("https://sharefs.tx.kugou.com/b.mp3")
        );
        assert_eq!(
            pick_login_url(&json!({ "play_url": "https://sharefs.tx.kugou.com/c.mp3" })).as_deref(),
            Some("https://sharefs.tx.kugou.com/c.mp3")
        );

        // 试听片段不算成功：is_free_part=1
        assert!(pick_login_url(&json!({
            "is_free_part": 1,
            "play_url": "https://fsandroid.tx.kugou.com/trial/a.mp3",
        }))
        .is_none());
        // 试听片段不算成功：URL 里带 trial（此时仍然可以去看备选直链）
        assert!(pick_login_url(&json!({
            "is_free_part": 0,
            "play_url": "https://fsandroid.tx.kugou.com/trial/a.mp3",
        }))
        .is_none());
        assert_eq!(
            pick_login_url(&json!({
                "is_free_part": 0,
                "play_url": "https://fsandroid.tx.kugou.com/trial/a.mp3",
                "play_backup_url": ["https://sharefs.tx.kugou.com/full.mp3"],
            }))
            .as_deref(),
            Some("https://sharefs.tx.kugou.com/full.mp3")
        );

        // 空/占位值不算直链
        assert!(pick_login_url(&json!({ "play_url": "" })).is_none());
        assert!(pick_login_url(&json!({ "play_url": ["", "not-a-url"] })).is_none());
        assert!(pick_login_url(&json!({})).is_none());
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
        // 实测失败形态（wwwapi 风控）：分类为可诊断错误，保留原文便于排查
        let failure = ensure_success(&json!({ "status": 0, "err_code": 30020 })).unwrap_err();
        let error = failure_message(failure, None);
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
        let failure = ensure_success(&data).unwrap_err();
        assert!(matches!(failure, LinkFailure::Restricted), "{failure:?}");
        let error = failure_message(failure, None);
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
        let failure = ensure_success(&data).unwrap_err();
        assert!(matches!(failure, LinkFailure::Restricted), "{failure:?}");
        assert_eq!(failure_message(failure, None), PAID_RESTRICTED_MESSAGE);
    }

    #[test]
    fn paid_flags_alone_are_enough_to_report_paid_message() {
        // 只有权限位、没有 error 文案时也要能认出来（保守推断：搜索接口自报 privilege 非 0）
        for shape in [
            json!({ "status": 0, "privilege": 8 }),
            json!({ "status": 0, "pay_type": 3 }),
            json!({ "status": 0, "error": "该歌曲需要VIP" }),
        ] {
            let failure = ensure_success(&shape).unwrap_err();
            assert!(matches!(failure, LinkFailure::Restricted), "{failure:?}");
            assert_eq!(failure_message(failure, None), PAID_RESTRICTED_MESSAGE);
        }
    }

    #[test]
    fn unrelated_failure_keeps_diagnostic_and_is_not_paid() {
        let failure =
            ensure_success(&json!({ "status": 0, "err_code": 20001, "error": "hash 不存在" }))
                .unwrap_err();
        assert!(matches!(failure, LinkFailure::Diagnostic(_)), "{failure:?}");
        let error = failure_message(failure, None);
        assert!(error.contains("status=0"));
        assert!(error.contains("20001"));
        assert!(error.contains("hash 不存在"));
        assert_ne!(error, PAID_RESTRICTED_MESSAGE);
        // status=2 但拿到了直链 ⇒ 不是受限（避免把「有 url 的 status=2」误判成付费）
        let with_url = json!({
            "status": 2,
            "url": "http://fsandroid.tx.kugou.com/example.mp3",
        });
        assert!(ensure_success(&with_url).is_err());
        let error = failure_message(ensure_success(&with_url).unwrap_err(), None);
        assert_ne!(error, PAID_RESTRICTED_MESSAGE);
    }

    #[test]
    fn restricted_message_depends_on_configured_account() {
        let restricted = || ensure_success(&restricted_song_info_fixture()).unwrap_err();

        // 未配置账号：告诉用户去哪里填 Cookie
        let anonymous = failure_message(restricted(), None);
        assert_eq!(anonymous, PAID_RESTRICTED_MESSAGE);
        assert!(
            anonymous.contains("设置 → 平台账号"),
            "未配置账号的文案必须给出落地页：{anonymous}"
        );

        // 已配置（可用）账号：告诉用户查权益与 Cookie 有效期，而不是再让他去填一遍
        let account = logged_in_cookie();
        let configured = failure_message(restricted(), Some(&account));
        assert_eq!(configured, ACCOUNT_NO_PERMISSION_MESSAGE);
        assert!(
            configured.contains("权益") && configured.contains("未过期"),
            "已配置账号的文案必须指向权益与有效期：{configured}"
        );
        assert_ne!(anonymous, configured);

        // 只有设备字段的 Cookie 功能上等于没配置账号，文案必须回到「去填 Cookie」
        let device_only = KugouCookie::parse("kg_mid=mid-abc; kg_dfid=dfid-99");
        assert_eq!(
            failure_message(restricted(), Some(&device_only)),
            PAID_RESTRICTED_MESSAGE
        );
    }
}
