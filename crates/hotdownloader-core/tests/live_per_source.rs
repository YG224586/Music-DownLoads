//! 真网络、逐音源独立验收（默认全部 `#[ignore]`）。
//!
//! 运行方式：
//!
//! ```text
//! cargo test -p hotdownloader-core --test live_per_source -- --ignored --nocapture
//! ```
//!
//! 铁律：每个用例只走**该音源自己的取链链路**（经由 app 发布的真实代码路径
//! [`PlatformDownloadLinkProvider::fetch`]）。某个源拿不到就在该源自己这里结束，
//! 断言的是该平台自己的文案/域名，任何用例都不会去问第二个音源。
//!
//! 环境变量（全部可选；未设置的账号相关用例按 SKIP 处理，**不 panic**）：
//! - `KG_COOKIE`：酷狗整条 Cookie（token / userid / dfid / mid）
//! - `NE_COOKIE`：网易云整条 Cookie（MUSIC_U）
//! - `MIGU_COOKIE`：咪咕整条 Cookie（token / userId）
//! - `QQ_COOKIE`：QQ 音乐网页版 Cookie（uin + qqmusic_key 或 qm_keyst）
//! - `KUWO_RID`：覆盖酷我样例 rid（默认 `228908`，《晴天》320k mp3）
//! - `BILI_BVID`：覆盖 B 站样例视频号（默认 `BV1BZbSzZEGT`）
//! - `NE_FREE_ID`：覆盖网易云免费样例 id（默认 `34341360`，《梦中的婚礼》）
//!
//! 每个用例把证据打到 stdout：直链主机、HTTP 状态、字节数、前 8 字节 magic；
//! Cookie 只打印前 6 后 4 位。
//!
//! # 实测记录（2026-10-04，本机直连）
//!
//! - **匿名免费档位只到标准音质**：酷狗免费样本 `128.mp3`（`getSongInfo` 通道）
//!   能取到，同一首歌 `320.mp3`（`trackercdn` v2）被平台按付费曲拒绝；网易云免费
//!   样本 `128.mp3` 能取到，`320.mp3` 只回试听片段（`freeTrialInfo`）。正向用例因此
//!   断言标准音质，高档位用 `print_tier` 只记录不判定。
//! - 酷狗《稻香》mid `8909e180…|…|960399` 的 320k 档匿名即被拒（属付费曲），
//!   不作正向样本；正向样本改用探针里的免费曲（蓝心羽《晴天》）。
//! - 网易云 `347230`（《海阔天空》）`fee=1`：匿名 128k 只给 `freeTrialInfo`
//!   `start=0,end=45` 的 45 秒片段，被拒是**正确**行为；`186016`（《晴天》）匿名时
//!   `url` 为空、`br=0`，走「无可用直链」分支。正向样本用 `34341360`
//!   （《梦中的婚礼》`fee=0`，匿名 128k `freeTrialInfo=null`、`br=128000`）。
//! - B 站 `playurl` 的 DASH 音轨 `baseUrl` 会落在第三方 PCDN 镜像域名上
//!   （实测 `*.edge.mountaintoys.cn`），因此判定用「不含其它音源的域名指纹 +
//!   偏移 4 处是 `ftyp` 容器」，不写死 B 站自己的域名。
//! - **B 站 CDN 对裸请求（只有 Range）回 403 `text/html`**，带上 B 站接口自己用的
//!   桌面 Chrome UA + 视频页 Referer 才回 206。内置直链不带自定义请求头
//!   （`download/link.rs::tests::builtin_link_carries_no_custom_headers`），
//!   两个形状都在这里实测并打印，便于定位传输层是否需要补 UA。

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use hotdownloader_core::download::link::{DownloadLinkProvider, PlatformDownloadLinkProvider};
use hotdownloader_core::platforms::credentials::{PlatformCookies, PlatformCredentialSource};
use hotdownloader_core::platforms::qqmusic::credentials::{QqAuth, QqCredentialSource};
use hotdownloader_core::platforms::Platform;
use reqwest::Client;

/// 各音源自己的直链域名指纹（铁律：直链主机只能命中自己这一组）。
const KUGOU_HOSTS: [&str; 3] = ["kugou.com", "kglink", "kgcdn"];
const KUWO_HOSTS: [&str; 3] = ["kuwo.cn", "kuwo.com", "kwcdn"];
const NETEASE_HOSTS: [&str; 4] = ["126.net", "163.com", "netease", "music.163.com"];
const MIGU_HOSTS: [&str; 3] = ["migu.cn", "nf.migu.cn", "migu"];
const BILIBILI_HOSTS: [&str; 4] = [
    "bilibili.com",
    "bilivideo.com",
    "hdslb.com",
    "akamaized.net",
];
const QQ_HOSTS: [&str; 2] = ["qq.com", "y.qq.com"];

/// `(音源名, 自己的域名组)`；顺序即下面 `const` 索引。
const PLATFORM_HOSTS: [(&str, &[&str]); 6] = [
    ("酷狗", &KUGOU_HOSTS),
    ("酷我", &KUWO_HOSTS),
    ("网易云", &NETEASE_HOSTS),
    ("咪咕", &MIGU_HOSTS),
    ("哔哩哔哩", &BILIBILI_HOSTS),
    ("QQ 音乐", &QQ_HOSTS),
];

const KUGOU: usize = 0;
const KUWO: usize = 1;
const NETEASE: usize = 2;
const MIGU: usize = 3;
const BILIBILI: usize = 4;

/// 直链主机不得命中任何**其它**音源的域名指纹（跨源替换检测）。
fn assert_no_other_platform(own_index: usize, host: &str) {
    let (own_name, _) = PLATFORM_HOSTS[own_index];
    for (index, (other_name, fingerprints)) in PLATFORM_HOSTS.iter().enumerate() {
        if index == own_index {
            continue;
        }
        for fingerprint in *fingerprints {
            assert!(
                !host.contains(fingerprint),
                "{own_name} 的直链主机出现 {other_name} 的域名指纹「{fingerprint}」（疑似跨源替换）: {host}"
            );
        }
    }
}

/// 直链主机必须落在该音源自己的域名组里。
fn assert_own_platform(own_index: usize, host: &str) {
    let (own_name, fingerprints) = PLATFORM_HOSTS[own_index];
    assert!(
        fingerprints
            .iter()
            .any(|fingerprint| host.contains(fingerprint)),
        "{own_name} 的直链主机不属于该音源自己的域名组: {host}"
    );
}

/// B 站接口自用的桌面 Chrome UA（与 `platforms/bilibili/mod.rs::BILI_UA` 同值）。
const BILI_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

// ------------------------------------------------------------------ 凭据来源

/// QQ 音乐的账号走「设置 → 平台账号」里粘贴的 `QQ_COOKIE`（由
/// [`EnvPlatformCredentialSource`] 提供）。这里恒为 `None`：既不构造 [`QqAuth`]，
/// 也不去碰 QQ 登录文件，从而覆盖「用户填了 Cookie / 没填 Cookie」两条真实分支。
struct EnvQqCredentialSource;

impl QqCredentialSource for EnvQqCredentialSource {
    fn current(&self) -> BoxFuture<'_, Result<Option<QqAuth>, String>> {
        Box::pin(async { Ok(None) })
    }
}

/// 平台账号 Cookie 直接读环境变量，等价于用户在设置页「填了 / 没填」两种状态。
struct EnvPlatformCredentialSource;

fn env_value(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

impl PlatformCredentialSource for EnvPlatformCredentialSource {
    fn cookies(&self) -> BoxFuture<'_, Result<PlatformCookies, String>> {
        let cookies = PlatformCookies {
            qq: env_value("QQ_COOKIE"),
            kugou: env_value("KG_COOKIE"),
            netease: env_value("NE_COOKIE"),
            migu: env_value("MIGU_COOKIE"),
        };
        Box::pin(async move { Ok(cookies) })
    }
}

// ------------------------------------------------------------------ 通用工具

fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("构造 HTTP 客户端失败")
}

fn provider() -> PlatformDownloadLinkProvider {
    PlatformDownloadLinkProvider::with_client(
        http_client(),
        Arc::new(EnvQqCredentialSource),
        Arc::new(EnvPlatformCredentialSource),
    )
}

/// 只取该音源自己的链路（`PlatformDownloadLinkProvider::fetch`），不接触别的音源。
async fn fetch_link(platform: Platform, mid: &str, filename: &str) -> Result<String, String> {
    let provider = provider();
    provider
        .fetch(platform, mid, filename)
        .await
        .map(|link| link.url)
}

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_else(|| "(无法解析)".to_string())
}

/// 打码：只留前 6 位与后 4 位。
fn mask(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 10 {
        return format!("(长度 {}，已隐藏)", chars.len());
    }
    let head: String = chars.iter().take(6).collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}(len={})", chars.len())
}

fn report_cookies() {
    for name in ["KG_COOKIE", "NE_COOKIE", "MIGU_COOKIE", "QQ_COOKIE"] {
        match env_value(name) {
            Some(value) => println!("   凭据 {name} = {}", mask(&value)),
            None => println!("   凭据 {name} = (未设置 → 该平台按自己的匿名能力取链)"),
        }
    }
}

/// 标注为非判定的档位探测：只打印平台返回，不参与 PASS/FAIL。
async fn print_tier(platform: Platform, mid: &str, filename: &str) {
    match fetch_link(platform, mid, filename).await {
        Ok(url) => println!(
            "   档位观察 {filename} → 取到直链（主机={}）",
            host_of(&url)
        ),
        Err(error) => println!("   档位观察 {filename} → 平台原文: {error}"),
    }
}

struct ByteProbe {
    status: u16,
    len: usize,
    content_type: String,
    bytes: Vec<u8>,
}

impl ByteProbe {
    fn magic_hex(&self) -> String {
        self.bytes
            .iter()
            .take(8)
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn magic_ascii(&self) -> String {
        self.bytes
            .iter()
            .take(8)
            .map(|byte| {
                if (0x20..0x7f).contains(byte) {
                    *byte as char
                } else {
                    '.'
                }
            })
            .collect()
    }

    fn is_ok(&self) -> bool {
        self.status == 200 || self.status == 206
    }

    fn starts_with(&self, prefix: &[u8]) -> bool {
        self.bytes.len() >= prefix.len() && &self.bytes[..prefix.len()] == prefix
    }

    /// m4a 的前 4 字节是 box 长度（可变），紧跟其后的 `ftyp` 才是容器指纹。
    fn has_ftyp_box(&self) -> bool {
        self.bytes.len() >= 12 && &self.bytes[4..8] == b"ftyp"
    }
}

/// Range 取前 4KB，证明拿到的确实是可下载的音频字节（真网络）。
async fn probe_bytes(
    url: &str,
    referer: Option<&str>,
    user_agent: Option<&str>,
) -> Result<ByteProbe, String> {
    let mut request = http_client()
        .get(url)
        .header(reqwest::header::RANGE, "bytes=0-4095");
    if let Some(referer) = referer {
        request = request.header(reqwest::header::REFERER, referer);
    }
    if let Some(user_agent) = user_agent {
        request = request.header(reqwest::header::USER_AGENT, user_agent);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("字节校验请求失败: {error}"))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("(无)")
        .to_string();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取字节失败: {error}"))?
        .to_vec();
    Ok(ByteProbe {
        status,
        len: bytes.len(),
        content_type,
        bytes,
    })
}

fn print_probe(label: &str, probe: &ByteProbe) {
    println!(
        "   {label} HTTP {} 字节数={} Content-Type={} magic(hex)={} magic(ascii)={}",
        probe.status,
        probe.len,
        probe.content_type,
        probe.magic_hex(),
        probe.magic_ascii()
    );
}

fn assert_audio_bytes(probe: &ByteProbe, platform: &str) {
    assert!(
        probe.is_ok(),
        "{platform} 字节校验 HTTP 状态异常: {}",
        probe.status
    );
    assert!(probe.len > 0, "{platform} 没有取回任何字节");
}

// ------------------------------------------------------------------ 一、能取到直链的五个音源

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn kugou_free_song_returns_playable_link() {
    // 「蓝心羽 - 晴天」：探针脚本（`_dev/unlock-probe/per-source-e2e.mjs`）里的
    // 免费样本（privilege=0 / pay_type=0），mid = 128k|320k|无损|专辑 id。
    const MID: &str =
        "48c685f679ffc7cf08b8a8341ca9db44|28f83bbd8cab043895039e98366ad2b4|857879479e698d729640a5716dc3e56f|79360569";
    println!("[酷狗/免费] mid={MID} filename=128.mp3");
    report_cookies();

    let url = fetch_link(Platform::Kugou, MID, "128.mp3")
        .await
        .unwrap_or_else(|error| {
            panic!("酷狗免费曲应能按匿名能力取到 128k 直链，实际返回: {error}")
        });
    let host = host_of(&url);
    println!("[酷狗/免费] 128.mp3 直链主机={host}（完整 URL 带签名，不打印）");
    assert_own_platform(KUGOU, &host);
    assert_no_other_platform(KUGOU, &host);

    let probe = probe_bytes(&url, None, None)
        .await
        .expect("酷狗直链字节校验");
    print_probe("[酷狗/免费]", &probe);
    assert_audio_bytes(&probe, "酷狗");
    assert!(
        probe.starts_with(b"ID3"),
        "酷狗 mp3 应以 ID3 开头，实际 magic={}",
        probe.magic_hex()
    );

    // 同一首歌的高档位匿名行为：只记录，不判定（免费曲的 320k/无损可能需登录）。
    print_tier(Platform::Kugou, MID, "320.mp3").await;
    print_tier(Platform::Kugou, MID, "2000.flac").await;
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn netease_free_song_returns_playable_link() {
    // 《梦中的婚礼》`fee=0`：匿名 128k `freeTrialInfo=null`、`br=128000`（探针实测）。
    let song_id = env_value("NE_FREE_ID").unwrap_or_else(|| "34341360".to_string());
    println!("[网易云/免费] id={song_id} filename=128.mp3");
    report_cookies();

    let url = fetch_link(Platform::Netease, &song_id, "128.mp3")
        .await
        .unwrap_or_else(|error| {
            panic!("网易云免费曲应能按匿名能力取到标准音质直链，实际返回: {error}")
        });
    let host = host_of(&url);
    println!("[网易云/免费] 128.mp3 直链主机={host}");
    assert_own_platform(NETEASE, &host);
    assert_no_other_platform(NETEASE, &host);

    let probe = probe_bytes(&url, None, None)
        .await
        .expect("网易云直链字节校验");
    print_probe("[网易云/免费]", &probe);
    assert_audio_bytes(&probe, "网易云");

    // 匿名 320k 实测只回试听片段 → 只记录，不判定。
    print_tier(Platform::Netease, &song_id, "320.mp3").await;
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn kuwo_song_returns_playable_link() {
    let rid = env_value("KUWO_RID").unwrap_or_else(|| "228908".to_string());
    println!("[酷我/《晴天》] rid={rid} filename=320.mp3");
    report_cookies();

    let url = fetch_link(Platform::Kuwo, &rid, "320.mp3")
        .await
        .unwrap_or_else(|error| panic!("酷我自带链路应能取到直链，实际返回: {error}"));
    let host = host_of(&url);
    println!("[酷我/《晴天》] 直链主机={host}（完整 URL 带签名，不打印）");
    assert_own_platform(KUWO, &host);
    assert_no_other_platform(KUWO, &host);

    let probe = probe_bytes(&url, None, None)
        .await
        .expect("酷我直链字节校验");
    print_probe("[酷我/《晴天》]", &probe);
    assert_audio_bytes(&probe, "酷我");
    assert!(
        probe.starts_with(b"ID3"),
        "酷我 mp3 应以 ID3 开头，实际 magic={}",
        probe.magic_hex()
    );
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn bilibili_audio_track_returns_playable_link() {
    let bvid = env_value("BILI_BVID").unwrap_or_else(|| "BV1BZbSzZEGT".to_string());
    println!("[哔哩哔哩/DASH 音轨] bvid={bvid} filename=192kaac.m4a");
    report_cookies();

    let url = fetch_link(Platform::Bilibili, &bvid, "192kaac.m4a")
        .await
        .unwrap_or_else(|error| panic!("B 站自带链路应能取到音轨直链，实际返回: {error}"));
    let host = host_of(&url);
    println!("[哔哩哔哩/DASH 音轨] 直链主机={host}（由 api.bilibili.com 的 playurl 给出）");
    // B 站会把音轨分发到第三方 PCDN 镜像域名，故只做「没有换到别的音源」检测，
    // 不写死 B 站自己的域名。
    assert_no_other_platform(BILIBILI, &host);

    // (a) 内置直链在传输层的形状：只带 Range（`DownloadLink.headers` 为空）。
    let bare = probe_bytes(&url, None, None)
        .await
        .expect("B 站直链字节校验（裸请求）");
    print_probe("[哔哩哔哩/裸请求(仅 Range)]", &bare);

    // (b) B 站接口自用的形状：桌面 Chrome UA + 视频页 Referer。
    let referer = format!("https://www.bilibili.com/video/{bvid}");
    let with_bili_headers = probe_bytes(&url, Some(&referer), Some(BILI_UA))
        .await
        .expect("B 站直链字节校验（UA+Referer）");
    print_probe("[哔哩哔哩/UA+Referer]", &with_bili_headers);

    let accepted = if with_bili_headers.is_ok() {
        &with_bili_headers
    } else {
        &bare
    };
    assert_audio_bytes(accepted, "哔哩哔哩");
    assert!(
        accepted.has_ftyp_box(),
        "B 站 m4a 应在偏移 4 处出现 ftyp box，实际 magic(hex)={}",
        accepted.magic_hex()
    );
    if !bare.is_ok() {
        println!(
            "   ⚠ 裸请求（应用内置直链的传输层形状）被 CDN 拒绝：HTTP {} —— 传输层可能需要补 UA",
            bare.status
        );
    }
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn migu_free_song_returns_playable_link() {
    // 探针脚本判定为非 VIP 独占的免费样本（MIGU_FREE）。
    const MID: &str = "60054704537|600919000007741344";
    println!("[咪咕/免费] mid={MID} filename=PQ.mp3");
    report_cookies();

    let url = fetch_link(Platform::Migu, MID, "PQ.mp3")
        .await
        .unwrap_or_else(|error| panic!("咪咕免费曲应能按匿名能力取到直链，实际返回: {error}"));
    let host = host_of(&url);
    println!("[咪咕/免费] 直链主机={host}");
    assert_own_platform(MIGU, &host);
    assert_no_other_platform(MIGU, &host);

    let probe = probe_bytes(&url, None, None)
        .await
        .expect("咪咕直链字节校验");
    print_probe("[咪咕/免费]", &probe);
    assert_audio_bytes(&probe, "咪咕");
    assert!(
        probe.starts_with(b"ID3"),
        "咪咕 mp3 应以 ID3 开头，实际 magic={}",
        probe.magic_hex()
    );
}

// ------------------------------------------------------------------ 二、付费/受限曲目必须回该平台自己的拒绝文案

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn kugou_paid_song_reports_its_own_rejection() {
    const MID: &str =
        "b3a52a7a958bf0aed0ebfba2e9a818b7|1b56126a8a03924f1dd066259c095cbc|0a69169202de95aaf24a9944ccf0730d|966846";
    println!("[酷狗/付费《晴天》] mid={MID} filename=320.mp3");
    report_cookies();

    match (
        fetch_link(Platform::Kugou, MID, "320.mp3").await,
        env_value("KG_COOKIE"),
    ) {
        (Ok(url), _) => println!(
            "[酷狗/付费《晴天》] 取到直链（主机={}）——有账号权益时属正常，不判定失败",
            host_of(&url)
        ),
        (Err(error), None) => {
            println!("[酷狗/付费《晴天》] 匿名拒绝原文: {error}");
            assert!(error.contains("酷狗"), "拒绝文案应带平台名: {error}");
            assert!(
                error.contains("设置 → 平台账号"),
                "未配置账号时应指引去设置页填 Cookie: {error}"
            );
        }
        (Err(error), Some(_)) => {
            println!("[酷狗/付费《晴天》] 带 KG_COOKIE 的返回（不判定失败）: {error}")
        }
    }
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn netease_paid_song_reports_its_own_rejection() {
    // 《海阔天空》`fee=1`：匿名 128k 只回 45 秒试听片段（`freeTrialInfo` start=0,end=45）。
    const SONG_ID: &str = "347230";
    println!("[网易云/付费《海阔天空》] id={SONG_ID} filename=128.mp3");
    report_cookies();

    match (
        fetch_link(Platform::Netease, SONG_ID, "128.mp3").await,
        env_value("NE_COOKIE"),
    ) {
        (Ok(url), _) => println!(
            "[网易云/付费《海阔天空》] 取到直链（主机={}）——有账号权益时属正常，不判定失败",
            host_of(&url)
        ),
        (Err(error), None) => {
            println!("[网易云/付费《海阔天空》] 匿名拒绝原文: {error}");
            assert!(error.contains("网易云"), "拒绝文案应带平台名: {error}");
            for forbidden in ["其它音源", "换源", "替代音源"] {
                assert!(!error.contains(forbidden), "拒绝文案不得跨源: {error}");
            }
        }
        (Err(error), Some(_)) => {
            println!("[网易云/付费《海阔天空》] 带 NE_COOKIE 的返回（不判定失败）: {error}")
        }
    }
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn netease_vip_song_reports_its_own_rejection() {
    // 《晴天》186016：匿名时接口不回 `url`（`br=0`），走「无可用直链」分支。
    println!("[网易云/VIP《晴天》] id=186016 filename=320.mp3");
    report_cookies();

    match (
        fetch_link(Platform::Netease, "186016", "320.mp3").await,
        env_value("NE_COOKIE"),
    ) {
        (Ok(url), _) => println!(
            "[网易云/VIP《晴天》] 取到直链（主机={}）——有账号权益时属正常，不判定失败",
            host_of(&url)
        ),
        (Err(error), None) => {
            println!("[网易云/VIP《晴天》] 匿名拒绝原文: {error}");
            assert!(error.contains("网易云"), "拒绝文案应带平台名: {error}");
            for forbidden in ["其它音源", "换源", "替代音源"] {
                assert!(!error.contains(forbidden), "拒绝文案不得跨源: {error}");
            }
        }
        (Err(error), Some(_)) => {
            println!("[网易云/VIP《晴天》] 带 NE_COOKIE 的返回（不判定失败）: {error}")
        }
    }
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn migu_vip_song_reports_its_own_rejection() {
    const MID: &str = "60054701923|600902000006889366";
    println!("[咪咕/VIP 独占《晴天》] mid={MID} filename=PQ.mp3");
    report_cookies();

    match (
        fetch_link(Platform::Migu, MID, "PQ.mp3").await,
        env_value("MIGU_COOKIE"),
    ) {
        (Ok(url), _) => println!(
            "[咪咕/VIP 独占《晴天》] 取到直链（主机={}）——有账号权益时属正常，不判定失败",
            host_of(&url)
        ),
        (Err(error), None) => {
            println!("[咪咕/VIP 独占《晴天》] 匿名拒绝原文: {error}");
            assert!(error.contains("咪咕"), "拒绝文案应带平台名: {error}");
            assert!(
                error.contains("设置 → 平台账号"),
                "未配置账号时应指引去设置页填 Cookie: {error}"
            );
        }
        (Err(error), Some(_)) => {
            println!("[咪咕/VIP 独占《晴天》] 带 MIGU_COOKIE 的返回: {error}")
        }
    }
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn migu_account_tiers_only_run_with_cookie() {
    println!("[咪咕/账号音质档] 仅当 MIGU_COOKIE 存在时探测 HQ.mp3 / SQ.flac");
    let Some(_cookie) = env_value("MIGU_COOKIE") else {
        println!("   SKIP：未设置 MIGU_COOKIE，跳过账号音质档探测");
        return;
    };
    const MID: &str = "60054701923|600902000006889366";
    print_tier(Platform::Migu, MID, "HQ.mp3").await;
    print_tier(Platform::Migu, MID, "SQ.flac").await;
}

#[tokio::test]
#[ignore = "真网络用例：加 -- --ignored --nocapture 才执行"]
async fn qq_anonymous_request_reports_its_own_rejection() {
    const MID: &str = "0035GveV3i9dBM";
    const FILENAME: &str = "C4000035GveV3i9dBM.m4a";
    println!("[QQ 音乐/匿名] mid={MID} filename={FILENAME}");
    report_cookies();

    match (
        fetch_link(Platform::QqMusic, MID, FILENAME).await,
        env_value("QQ_COOKIE"),
    ) {
        (Ok(url), _) => println!(
            "[QQ 音乐/匿名] 取到直链（主机={}）——账号有权益时属正常，不判定失败",
            host_of(&url)
        ),
        (Err(error), None) => {
            println!("[QQ 音乐/匿名] 匿名拒绝原文: {error}");
            assert!(error.contains("QQ 音乐"), "拒绝文案应带平台名: {error}");
            assert!(
                error.contains("设置 → 平台账号"),
                "匿名被拒时应指引去设置页填 Cookie: {error}"
            );
        }
        (Err(error), Some(_)) => {
            println!("[QQ 音乐] 带 QQ_COOKIE 的返回（不判定失败）: {error}")
        }
    }
}
