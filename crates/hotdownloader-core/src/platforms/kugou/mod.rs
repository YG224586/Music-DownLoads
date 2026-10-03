//! 酷狗音乐平台：搜索与直链获取。
//!
//! 探测证据见 `_dev/probe/kugou-probe.mjs`、`_dev/probe/kugou-probe.log`、
//! `_dev/probe/kugou-probe-hq.log`。
//!
//! 实测可用的三个端点：
//! - 搜索：`http://mobilecdn.kugou.com/api/v3/search/song`（只有 http 域名可用，
//!   https 变体分别报 `ENOTFOUND` / `ERR_TLS_CERT_ALTNAME_INVALID`；Android 已开启明文流量）。
//! - 128k mp3：`https://m.kugou.com/app/i/getSongInfo.php?cmd=playInfo`（无需签名，
//!   但接口无视请求的档位，只会给 128k）。
//! - 320k mp3 / 无损 flac：`https://trackercdn.kugou.com/i/v2/`（需要
//!   `key = md5(小写 hash + "kgcloudv2")` 签名，见 [`sign`]）。
//!
//! 未实测通的档位（hires / 全景声 / 蝰蛇母带）不上架：搜索响应里没有对应 hash 字段，
//! 拿不到直链时宁可不展示，避免误导用户。
//!
//! # 付费曲目
//!
//! 付费/VIP 曲目由酷狗服务端按账号权益签发直链，匿名一律拿不到（实测
//! `trackercdn` v1 回 `The Resource Needs to be Paid`、`yy/index.php?r=play/getdata`
//! 与 `wwwapi play/songinfo` 回 `err_code=30020`「需要登录」）。因此本平台提供
//! **酷狗自己的账号 Cookie** 入口（[`credentials`]）：填入后按登录态请求
//! `yy/index.php?r=play/getdata`，由酷狗签发该账号有权拿到的直链。
//! 拿不到时只报酷狗自己的错误，不换到其它音源。

pub mod credentials;
pub mod link;
pub mod parser;
pub mod search;
pub mod sign;
