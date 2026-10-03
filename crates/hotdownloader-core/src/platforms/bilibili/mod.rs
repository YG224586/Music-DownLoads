//! 哔哩哔哩（bili）内置音源：视频区 DASH 音轨方案。
//!
//! 背景结论（2026-10 实测，探测脚本 `_dev/probe/bilibili-probe.mjs`）：
//! - B 站音频区旧搜索接口 `audio/music-service-c/s` 已废弃（任何 search_type
//!   均返回空 result），新 wbi 搜索无签名会被 -1200 风控降级；
//! - 可行路径 = 老版 web 搜索（非 wbi）→ 视频区结果 → `view` 拿 cid →
//!   `playurl(fnval=16)` 取 DASH 音轨 → 192kbps 轨（id=30280）即 `bili192`。
//!
//! 注意：本源的内容主体是**视频的音轨**（UP 主投稿/搬运），不是官方音频区
//! 上传；标题/歌手尽力从视频元数据补全（`artist` = UP 主名）。音质映射
//! `bili192 → 192kaac`，容器为 m4a/AAC（`filename` 扩展名必须是 `.m4a`）。

pub mod link;
pub mod parser;
pub mod search;

use serde_json::Value;

use crate::platforms::CLIENT;

/// B 站 web 接口对 UA 较敏感，用新版 Chrome 桌面 UA。
pub(crate) const BILI_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// `finger/spi` 拿 buvid3/buvid4 指纹 cookie（搜索/取链共用）。
pub(crate) async fn fetch_buvid_cookie() -> Result<String, String> {
    let root = http_get_json("https://api.bilibili.com/x/frontend/finger/spi", "").await?;
    let b3 = root["data"]["b_3"].as_str().unwrap_or_default();
    let b4 = root["data"]["b_4"].as_str().unwrap_or_default();
    if b3.is_empty() {
        return Err("网络错误: 获取 bilibili 指纹失败".to_string());
    }
    Ok(format!("buvid3={}; buvid4={}", b3, b4))
}

/// 用平台共享 CLIENT 发 GET（带 UA/Referer/cookie），解析 JSON。
pub(crate) async fn http_get_json(url: &str, cookie: &str) -> Result<Value, String> {
    http_get_json_with(&CLIENT, url, cookie).await
}

/// 用指定 client（下载链路传入的）发 GET，解析 JSON。
pub(crate) async fn http_get_json_with(
    client: &reqwest::Client,
    url: &str,
    cookie: &str,
) -> Result<Value, String> {
    let mut req = client
        .get(url)
        .header("User-Agent", BILI_UA)
        .header("Referer", "https://www.bilibili.com/");
    if !cookie.is_empty() {
        req = req.header("Cookie", cookie);
    }
    let text = req
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
}
