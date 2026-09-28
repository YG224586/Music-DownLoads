//! 独立服务的访问认证。配置在启动时确定，所有 API 和 SSE 使用同一种模式。

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

pub enum AccessAuth {
    None,
    Token(String),
    Password { username: String, password: String },
}

impl AccessAuth {
    pub fn from_env() -> Result<Self, String> {
        Self::from_values(
            std::env::var("AUTH_USERNAME").unwrap_or_default(),
            std::env::var("AUTH_PASSWORD").unwrap_or_default(),
            std::env::var("HOTDOWNLOADER_TOKEN").unwrap_or_default(),
        )
    }

    fn from_values(username: String, password: String, token: String) -> Result<Self, String> {
        if !username.is_empty() || !password.is_empty() {
            if username.is_empty() || password.is_empty() {
                return Err("AUTH_USERNAME 和 AUTH_PASSWORD 必须同时设置".into());
            }
            if username.contains(':') {
                return Err("AUTH_USERNAME 不能包含冒号".into());
            }
            return Ok(Self::Password { username, password });
        }
        if !token.is_empty() {
            Ok(Self::Token(token))
        } else {
            Ok(Self::None)
        }
    }

    pub fn mode(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Token(_) => "token",
            Self::Password { .. } => "password",
        }
    }

    pub fn valid_for_external_access(&self) -> bool {
        match self {
            Self::None => false,
            Self::Token(token) => token.len() >= 16,
            Self::Password { .. } => true,
        }
    }

    pub fn accepts(&self, authorization: Option<&str>) -> bool {
        match self {
            Self::None => true,
            Self::Token(expected) => authorization
                .and_then(|value| value.strip_prefix("Bearer "))
                .is_some_and(|supplied| constant_time_eq(expected, supplied)),
            Self::Password { username, password } => authorization
                .and_then(|value| value.strip_prefix("Basic "))
                .and_then(|encoded| STANDARD.decode(encoded).ok())
                .and_then(|decoded| String::from_utf8(decoded).ok())
                .and_then(|credentials| {
                    credentials
                        .split_once(':')
                        .map(|(name, secret)| (name.to_string(), secret.to_string()))
                })
                .is_some_and(|(name, secret)| {
                    constant_time_eq(username, &name) & constant_time_eq(password, &secret)
                }),
        }
    }
}

fn constant_time_eq(expected: &str, supplied: &str) -> bool {
    let expected = expected.as_bytes();
    let supplied = supplied.as_bytes();
    let mut difference = expected.len() ^ supplied.len();
    for (a, b) in expected.iter().zip(supplied) {
        difference |= usize::from(a ^ b);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::AccessAuth;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;

    #[test]
    fn password_configuration_takes_priority_over_token() {
        let auth =
            AccessAuth::from_values("admin".into(), "短密码".into(), "old-token".into()).unwrap();
        assert_eq!(auth.mode(), "password");
        assert!(auth.valid_for_external_access());
        assert!(auth.accepts(Some(&format!("Basic {}", STANDARD.encode("admin:短密码")))));
        assert!(!auth.accepts(Some("Bearer old-token")));
        assert!(!auth.accepts(Some(&format!("Basic {}", STANDARD.encode("admin:wrong")))));
    }

    #[test]
    fn incomplete_password_configuration_does_not_fall_back_to_token() {
        assert!(AccessAuth::from_values("admin".into(), "".into(), "old-token".into()).is_err());
        assert!(AccessAuth::from_values("".into(), "secret".into(), "old-token".into()).is_err());
        assert!(AccessAuth::from_values("bad:name".into(), "secret".into(), "".into()).is_err());
    }

    #[test]
    fn token_configuration_remains_supported() {
        let auth =
            AccessAuth::from_values("".into(), "".into(), "1234567890123456".into()).unwrap();
        assert_eq!(auth.mode(), "token");
        assert!(auth.valid_for_external_access());
        assert!(auth.accepts(Some("Bearer 1234567890123456")));
        assert!(!auth.accepts(Some("Bearer wrong")));
        assert!(!auth.accepts(Some(&format!("Basic {}", STANDARD.encode("admin:secret")))));
    }
}
