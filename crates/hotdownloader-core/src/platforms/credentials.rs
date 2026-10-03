//! 各内置音源的账号凭据端口。
//!
//! # 为什么需要这个端口
//!
//! 付费（VIP / 数字专辑）曲目的直链由**各音源自己的服务端**按账号权益签发，匿名请求
//! 一律拿不到。实测证据（`_dev/unlock-probe/per-source-probe*.mjs`，2026-10）：
//! - 酷狗 `trackercdn.kugou.com/i/?cmd=4&...` 对受限曲回
//!   `{"status":0,"error":"The Resource Needs to be Paid"}`（`key=md5(hash+"kgcloud")` 合法）；
//! - 酷狗 `wwwapi.kugou.com/yy/index.php?r=play/getdata` 与 `wwwapi.kugou.com/play/songinfo`
//!   对**免费曲也一样**回 `err_code=30020`（= 需要登录），带随机设备对无效；
//! - 网易云 `music.163.com/song/media/outer/url?id=<VIP 曲>.mp3` 302 到 `/404`，
//!   而免费曲 302 到真实 CDN；
//! - 咪咕 `listenSong.do` 对 VIP 独占曲回「暂不提供试听地址」；
//! - QQ 音乐 `u.y.qq.com/cgi-bin/musicu.fcg` 的 `vkey.GetVkeyServer.CgiGetVkey` 对匿名请求
//!   一律回 `result=104003`、`purl=""`（`_dev/unlock-probe/qq-anon-probe.mjs`，2026-10），
//!   匿名通道已被平台关闭。
//!
//! 因此「每个音源下自己的曲目」的正确做法是：该音源自己的链路 + 该音源自己的账号凭据。
//! 本模块只承载凭据，**不做任何跨音源替换**——缺凭据时各音源按自身匿名能力降级，
//! 拿不到就报该音源自己的确定性错误。
//!
//! 凭据只在本机保存（Tauri Store 的 `settings.platformCookies`），不进入任务记录、
//! 不写日志、不随进度事件回传前端。

use futures_util::future::BoxFuture;

/// 四个需要登录态才能解锁付费曲目的音源各自的原始 Cookie 串。
///
/// `None` 表示该音源未配置账号 → 按匿名能力降级。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformCookies {
    /// QQ 音乐：整条 Cookie（至少含 `uin`，以及 `qqmusic_key` / `qm_keyst` 之一）。
    pub qq: Option<String>,
    /// 酷狗：浏览器或抓包拿到的整条 Cookie（至少含 `token`、`userid`、`dfid`、`mid`）。
    pub kugou: Option<String>,
    /// 网易云：整条 Cookie（至少含 `MUSIC_U`）。
    pub netease: Option<String>,
    /// 咪咕：整条 Cookie（至少含 `token` / `userId`）。
    pub migu: Option<String>,
}

impl PlatformCookies {
    /// 全空 = 纯匿名下载（不读取任何账号）。
    pub fn is_empty(&self) -> bool {
        self.qq().is_none()
            && self.kugou().is_none()
            && self.netease().is_none()
            && self.migu().is_none()
    }

    /// 去掉首尾空白后的 QQ 音乐 Cookie；空白串按「未配置」处理。
    pub fn qq(&self) -> Option<&str> {
        normalize(self.qq.as_deref())
    }

    /// 去掉首尾空白后的酷狗 Cookie；空白串按「未配置」处理。
    pub fn kugou(&self) -> Option<&str> {
        normalize(self.kugou.as_deref())
    }

    /// 去掉首尾空白后的网易云 Cookie。
    pub fn netease(&self) -> Option<&str> {
        normalize(self.netease.as_deref())
    }

    /// 去掉首尾空白后的咪咕 Cookie。
    pub fn migu(&self) -> Option<&str> {
        normalize(self.migu.as_deref())
    }
}

fn normalize(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// 下载任务每次取链时读取当前账号凭据；运行时（Tauri / 测试）提供实现。
///
/// 实现方不得把 Cookie 写进日志；错误只用于诊断「设置读取失败」这类本机问题。
pub trait PlatformCredentialSource: Send + Sync {
    fn cookies(&self) -> BoxFuture<'_, Result<PlatformCookies, String>>;
}

/// 无账号实现：所有音源都按匿名降级（离线测试与 CLI 复用）。
pub struct AnonymousCredentialSource;

impl PlatformCredentialSource for AnonymousCredentialSource {
    fn cookies(&self) -> BoxFuture<'_, Result<PlatformCookies, String>> {
        Box::pin(async { Ok(PlatformCookies::default()) })
    }
}

#[cfg(test)]
mod tests {
    use super::PlatformCookies;

    #[test]
    fn whitespace_only_cookies_are_treated_as_anonymous() {
        let cookies = PlatformCookies {
            qq: Some("\t".into()),
            kugou: Some("   ".into()),
            netease: Some("\n".into()),
            migu: None,
        };
        assert!(cookies.is_empty());
        assert!(cookies.qq().is_none());
        assert!(cookies.kugou().is_none());
        assert!(cookies.netease().is_none());
        assert!(cookies.migu().is_none());
    }

    #[test]
    fn cookies_are_trimmed_and_exposed_verbatim() {
        let cookies = PlatformCookies {
            qq: Some("  uin=o42; qqmusic_key=k1  ".into()),
            kugou: Some("  token=abc; userid=42  ".into()),
            netease: Some("MUSIC_U=xyz".into()),
            migu: Some("token=migu".into()),
        };
        assert!(!cookies.is_empty());
        assert_eq!(cookies.qq(), Some("uin=o42; qqmusic_key=k1"));
        assert_eq!(cookies.kugou(), Some("token=abc; userid=42"));
        assert_eq!(cookies.netease(), Some("MUSIC_U=xyz"));
        assert_eq!(cookies.migu(), Some("token=migu"));
    }

    /// QQ 是后加的一档：只有它非空时同样不算匿名下载。
    #[test]
    fn qq_cookie_alone_is_not_anonymous() {
        let cookies = PlatformCookies {
            qq: Some("uin=o42; qm_keyst=legacy".into()),
            ..PlatformCookies::default()
        };
        assert!(!cookies.is_empty());
        assert_eq!(cookies.qq(), Some("uin=o42; qm_keyst=legacy"));
        assert!(cookies.kugou().is_none());
        assert!(cookies.netease().is_none());
        assert!(cookies.migu().is_none());
    }
}
