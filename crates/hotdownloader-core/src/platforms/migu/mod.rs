//! 咪咕音乐（Migu）内置音源。
//!
//! 与酷我、QQ 音乐一致，本模块只提供搜索与取链两个入口：
//!
//! - [`search::search_songs`]：调用咪咕 App 搜索接口，返回契约 §2 规定的歌曲 JSON；
//! - [`link::get_download_link`]：调用咪咕 `listenSong.do`，返回签名 CDN 直链。
//!
//! 平台注册（`Platform` 枚举、下载链路 match 臂、前端平台表）由上层维护，本模块不感知。
//!
//! 咪咕这两条接口都不需要自行计算签名，但都必须带自家 App 的 `channel` 请求头，
//! 否则会被网关拒绝（实测见 `_dev/probe/migu-probe-e1.log`）。

pub mod link;
pub mod parser;
pub mod search;

/// 咪咕 App 的 Android UA（搜索与取链共用）。
pub(crate) const UA_ANDROID: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

/// 咪咕客户端渠道号；缺失时接口常直接拒绝。
pub(crate) const CHANNEL: &str = "014000D";

/// 给咪咕 App 接口的请求补齐公共请求头。
pub(crate) fn app_request(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    request
        .header("User-Agent", UA_ANDROID)
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .header("channel", CHANNEL)
        .header("Referer", "https://app.c.nf.migu.cn/")
        .header("Origin", "https://app.c.nf.migu.cn")
}
