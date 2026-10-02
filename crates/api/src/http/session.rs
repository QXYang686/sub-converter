use axum::http::{header, HeaderMap, HeaderValue};

use super::error::ApiError;

pub const REFRESH_COOKIE_NAME: &str = "refresh_token";
pub const REFRESH_COOKIE_PATH: &str = "/api/auth";
pub const XHR_HEADER: &str = "x-requested-with";
pub const XHR_HEADER_VALUE: &str = "XMLHttpRequest";

pub fn is_secure_request(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

fn cookie_attributes(secure: bool, max_age_seconds: i64) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!(
        "HttpOnly; SameSite=Strict; Path={REFRESH_COOKIE_PATH}; Max-Age={max_age_seconds}{secure}"
    )
}

pub fn set_refresh_cookie(
    token: &str,
    max_age_seconds: i64,
    secure: bool,
) -> Result<HeaderValue, ApiError> {
    let value = format!(
        "{REFRESH_COOKIE_NAME}={token}; {}",
        cookie_attributes(secure, max_age_seconds)
    );
    HeaderValue::from_str(&value)
        .map_err(|err| ApiError::Internal(format!("invalid refresh cookie: {err}")))
}

pub fn clear_refresh_cookie(secure: bool) -> HeaderValue {
    let value = format!(
        "{REFRESH_COOKIE_NAME}=; {}",
        cookie_attributes(secure, 0)
    );
    HeaderValue::from_str(&value).expect("cleared cookie is always valid")
}

pub fn refresh_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    parse_cookie(cookies, REFRESH_COOKIE_NAME)
}

fn parse_cookie(header: &str, name: &str) -> Option<String> {
    header
        .split(';')
        .filter_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then(|| value.to_string())
        })
        .next()
}

pub fn require_xhr(headers: &HeaderMap) -> Result<(), ApiError> {
    let is_xhr = headers
        .get(XHR_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case(XHR_HEADER_VALUE))
        .unwrap_or(false);
    if is_xhr {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "missing XMLHttpRequest header".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with(name: &'static str, value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, HeaderValue::from_static(value));
        headers
    }

    #[test]
    fn parses_refresh_token_among_other_cookies() {
        let headers = headers_with("cookie", "a=1; refresh_token=tok-123; b=2");
        assert_eq!(
            refresh_token_from_headers(&headers).as_deref(),
            Some("tok-123")
        );
    }

    #[test]
    fn ignores_missing_cookie() {
        let headers = headers_with("cookie", "a=1; b=2");
        assert_eq!(refresh_token_from_headers(&headers), None);
        assert_eq!(refresh_token_from_headers(&HeaderMap::new()), None);
    }

    #[test]
    fn set_cookie_has_security_attributes() {
        let cookie = set_refresh_cookie("tok", 3600, true).unwrap();
        let cookie = cookie.to_str().unwrap();
        assert!(cookie.starts_with("refresh_token=tok;"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Path=/api/auth"));
        assert!(cookie.contains("Max-Age=3600"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn secure_attribute_is_optional() {
        let cookie = set_refresh_cookie("tok", 60, false).unwrap();
        assert!(!cookie.to_str().unwrap().contains("Secure"));
    }

    #[test]
    fn clear_cookie_expires_immediately() {
        let cookie = clear_refresh_cookie(true);
        let cookie = cookie.to_str().unwrap();
        assert!(cookie.starts_with("refresh_token=;"));
        assert!(cookie.contains("Max-Age=0"));
    }

    #[test]
    fn secure_request_detection() {
        let secure = headers_with("x-forwarded-proto", "HTTPS");
        assert!(is_secure_request(&secure));
        assert!(!is_secure_request(&HeaderMap::new()));
    }

    #[test]
    fn xhr_guard_accepts_only_matching_header() {
        assert!(require_xhr(&HeaderMap::new()).is_err());
        let matching = headers_with(XHR_HEADER, XHR_HEADER_VALUE);
        assert!(require_xhr(&matching).is_ok());
    }
}
