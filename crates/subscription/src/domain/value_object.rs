use std::fmt;

use uuid::Uuid;

use super::DomainError;

pub const SUBSCRIPTION_NAME_MIN_LEN: usize = 1;
pub const SUBSCRIPTION_NAME_MAX_LEN: usize = 64;
pub const SOURCE_URL_MAX_LEN: usize = 2048;
pub const PUBLICATION_SECRET_MIN_LEN: usize = 32;
pub const PUBLICATION_SECRET_MAX_LEN: usize = 128;
pub const PUBLICATION_MAX_SOURCES: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceId(Uuid);

impl SourceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainError::InvalidSourceId)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SourceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicationId(Uuid);

impl PublicationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainError::InvalidPublicationId)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for PublicationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PublicationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionName(String);

impl SubscriptionName {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        let length = trimmed.chars().count();
        let valid_length =
            (SUBSCRIPTION_NAME_MIN_LEN..=SUBSCRIPTION_NAME_MAX_LEN).contains(&length);
        let valid_chars = !trimmed.chars().any(char::is_control);
        if !valid_length || !valid_chars {
            return Err(DomainError::InvalidSubscriptionName);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SubscriptionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceUrl(String);

impl SourceUrl {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.len() > SOURCE_URL_MAX_LEN
            || trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(DomainError::InvalidSourceUrl);
        }

        let lower = trimmed.to_ascii_lowercase();
        let rest = if lower.starts_with("https://") {
            &trimmed[8..]
        } else if lower.starts_with("http://") {
            &trimmed[7..]
        } else {
            return Err(DomainError::InvalidSourceUrl);
        };

        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        if authority.is_empty() || authority.contains('@') {
            return Err(DomainError::InvalidSourceUrl);
        }

        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicationSecret(String);

impl PublicationSecret {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let length = raw.chars().count();
        let valid_length =
            (PUBLICATION_SECRET_MIN_LEN..=PUBLICATION_SECRET_MAX_LEN).contains(&length);
        let valid_chars = raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid_length || !valid_chars {
            return Err(DomainError::InvalidPublicationSecret);
        }
        Ok(Self(raw.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PublicationSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_through_string() {
        let source_id = SourceId::new();
        assert_eq!(SourceId::parse(&source_id.to_string()).unwrap(), source_id);
        let publication_id = PublicationId::new();
        assert_eq!(
            PublicationId::parse(&publication_id.to_string()).unwrap(),
            publication_id
        );
    }

    #[test]
    fn ids_reject_invalid_input() {
        assert_eq!(
            SourceId::parse("not-a-uuid"),
            Err(DomainError::InvalidSourceId)
        );
        assert_eq!(
            PublicationId::parse("not-a-uuid"),
            Err(DomainError::InvalidPublicationId)
        );
    }

    #[test]
    fn subscription_name_is_trimmed() {
        let name = SubscriptionName::new("  我的订阅  ").unwrap();
        assert_eq!(name.value(), "我的订阅");
    }

    #[test]
    fn subscription_name_rejects_invalid_values() {
        let too_long = "x".repeat(SUBSCRIPTION_NAME_MAX_LEN + 1);
        let cases = ["", "  ", too_long.as_str(), "bad\nname"];
        for raw in cases {
            assert_eq!(
                SubscriptionName::new(raw),
                Err(DomainError::InvalidSubscriptionName),
                "expected {raw:?} to be rejected"
            );
        }
    }

    #[test]
    fn source_url_accepts_http_and_https() {
        let cases = [
            "http://example.com/sub",
            "https://example.com/sub?token=abc#frag",
            "HTTPS://example.com/sub",
            " http://example.com ",
        ];
        for raw in cases {
            assert!(
                SourceUrl::new(raw).is_ok(),
                "expected {raw:?} to be accepted"
            );
        }
    }

    #[test]
    fn source_url_rejects_invalid_values() {
        let too_long = format!("https://example.com/{}", "x".repeat(SOURCE_URL_MAX_LEN));
        let cases = [
            "",
            "ftp://example.com",
            "example.com",
            "https://",
            "https://user:pass@example.com",
            "https://example.com/a b",
            too_long.as_str(),
        ];
        for raw in cases {
            assert_eq!(
                SourceUrl::new(raw),
                Err(DomainError::InvalidSourceUrl),
                "expected {raw:?} to be rejected"
            );
        }
    }

    #[test]
    fn publication_secret_validates_shape() {
        let valid = "a".repeat(PUBLICATION_SECRET_MIN_LEN);
        assert!(PublicationSecret::new(&valid).is_ok());
        let cases = [
            "short".to_string(),
            "a".repeat(PUBLICATION_SECRET_MAX_LEN + 1),
            "with space".repeat(4),
            "bad!".repeat(8),
        ];
        for raw in cases {
            assert_eq!(
                PublicationSecret::new(&raw),
                Err(DomainError::InvalidPublicationSecret),
                "expected {raw:?} to be rejected"
            );
        }
    }
}
