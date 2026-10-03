//! 酷狗账号 Cookie 的解析。
//!
//! 酷狗登录态下载链路需要四个字段，全部来自浏览器/抓包 Cookie：
//! - `token`：登录令牌（酷狗网页端与客户端都用它，缺失时接口回 `err_code=30020`）；
//! - `userid`（或 `KugouID`）：账号数字 ID；
//! - `dfid`（或 `kg_dfid`）：设备指纹；
//! - `mid`（或 `kg_mid`）：设备 mid。
//!
//! 解析器对键名大小写与首尾空白都不敏感，未知键原样忽略；整条 Cookie 也会原样随请求发送，
//! 因此新增字段（如 `kg_mid_temp`）不会被丢掉。

/// 从整条 Cookie 中抽出的酷狗登录态字段。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KugouCookie {
    raw: String,
    token: String,
    userid: String,
    dfid: String,
    mid: String,
}

impl KugouCookie {
    /// 解析整条 Cookie 串；缺失字段留空，不报错（未登录的 Cookie 仍可能凭设备字段可用）。
    pub fn parse(raw: &str) -> Self {
        let mut parsed = Self {
            raw: raw.trim().to_string(),
            ..Self::default()
        };
        for part in raw.split(';') {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key.as_str() {
                "token" => set_once(&mut parsed.token, value),
                "userid" | "kugouid" | "user_id" => set_once(&mut parsed.userid, value),
                "kg_dfid" | "dfid" => set_once(&mut parsed.dfid, value),
                "kg_mid" | "mid" => set_once(&mut parsed.mid, value),
                _ => {}
            }
        }
        parsed
    }

    /// 整条原始 Cookie，直接作为 `Cookie` 请求头发送。
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn userid(&self) -> &str {
        &self.userid
    }

    pub fn dfid(&self) -> &str {
        &self.dfid
    }

    pub fn mid(&self) -> &str {
        &self.mid
    }

    /// 是否具备发起「登录态」请求的最小字段。
    ///
    /// 实测：只有设备字段（`dfid`/`mid`）而没有 `token`+`userid` 时，酷狗接口照样回
    /// `err_code=30020`，所以在缺 token/userid 时不必浪费一次登录态请求。
    pub fn is_logged_in(&self) -> bool {
        !self.token.is_empty() && !self.userid.is_empty()
    }
}

fn set_once(slot: &mut String, value: &str) {
    if slot.is_empty() {
        *slot = value.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::KugouCookie;

    #[test]
    fn parses_browser_cookie_shape() {
        let cookie = KugouCookie::parse(
            "kg_mid=abc123; kg_dfid=df-99; token=tok-7; userid=1234567; KugouID=1234567; other=1",
        );
        assert_eq!(cookie.mid(), "abc123");
        assert_eq!(cookie.dfid(), "df-99");
        assert_eq!(cookie.token(), "tok-7");
        assert_eq!(cookie.userid(), "1234567");
        assert!(cookie.is_logged_in());
        assert_eq!(cookie.raw().split("; ").count(), 6);
    }

    #[test]
    fn falls_back_to_short_key_names() {
        let cookie = KugouCookie::parse("mid=M; dfid=D; token=T; user_id=U");
        assert_eq!(cookie.mid(), "M");
        assert_eq!(cookie.dfid(), "D");
        assert_eq!(cookie.token(), "T");
        assert_eq!(cookie.userid(), "U");
        assert!(cookie.is_logged_in());
    }

    #[test]
    fn device_only_cookie_is_not_logged_in() {
        let cookie = KugouCookie::parse("kg_mid=abc; kg_dfid=def");
        assert!(!cookie.is_logged_in());
        assert_eq!(cookie.mid(), "abc");
        assert_eq!(cookie.dfid(), "def");
    }

    #[test]
    fn empty_and_garbage_input_is_safe() {
        let cookie = KugouCookie::parse("   ");
        assert!(!cookie.is_logged_in());
        assert_eq!(cookie.raw(), "");
        let garbage = KugouCookie::parse("token=; =x; noise; userid=42");
        assert_eq!(garbage.token(), "");
        assert_eq!(garbage.userid(), "42");
        assert!(!garbage.is_logged_in());
    }

    #[test]
    fn first_value_wins_for_duplicate_keys() {
        let cookie = KugouCookie::parse("token=a; token=b; userid=1; userid=2");
        assert_eq!(cookie.token(), "a");
        assert_eq!(cookie.userid(), "1");
    }
}
