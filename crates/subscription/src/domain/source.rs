mod snapshot;

use std::fmt;

use user::UserId;
use uuid::Uuid;

use super::{DomainError, SubscriptionName};

pub use snapshot::{body_hash, SnapshotMeta, SourceExtraction, SourceSnapshot};

pub const SOURCE_URL_MAX_LEN: usize = 2048;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    id: SourceId,
    user_id: UserId,
    name: SubscriptionName,
    url: SourceUrl,
    enabled: bool,
    created_at: i64,
    updated_at: i64,
}

impl Source {
    pub fn create(
        id: SourceId,
        user_id: UserId,
        name: SubscriptionName,
        url: SourceUrl,
        now: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            name,
            url,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: SourceId,
        user_id: UserId,
        name: SubscriptionName,
        url: SourceUrl,
        enabled: bool,
        created_at: i64,
        updated_at: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            name,
            url,
            enabled,
            created_at,
            updated_at,
        }
    }

    pub fn rename(&mut self, name: SubscriptionName, now: i64) {
        if self.name != name {
            self.name = name;
            self.updated_at = now;
        }
    }

    pub fn change_url(&mut self, url: SourceUrl, now: i64) {
        if self.url != url {
            self.url = url;
            self.updated_at = now;
        }
    }

    pub fn set_enabled(&mut self, enabled: bool, now: i64) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.updated_at = now;
        }
    }

    pub fn id(&self) -> &SourceId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn name(&self) -> &SubscriptionName {
        &self.name
    }

    pub fn url(&self) -> &SourceUrl {
        &self.url
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_id_round_trips_through_string() {
        let source_id = SourceId::new();
        assert_eq!(SourceId::parse(&source_id.to_string()).unwrap(), source_id);
    }

    #[test]
    fn source_id_rejects_invalid_input() {
        assert_eq!(
            SourceId::parse("not-a-uuid"),
            Err(DomainError::InvalidSourceId)
        );
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

    fn sample_source() -> Source {
        Source::create(
            SourceId::new(),
            UserId::new(),
            SubscriptionName::new("机场 A").unwrap(),
            SourceUrl::new("https://example.com/sub").unwrap(),
            1_700_000_000,
        )
    }

    #[test]
    fn create_enables_and_initializes_timestamps() {
        let source = sample_source();
        assert!(source.enabled());
        assert_eq!(source.created_at(), 1_700_000_000);
        assert_eq!(source.updated_at(), 1_700_000_000);
    }

    #[test]
    fn changes_bump_updated_at() {
        let mut source = sample_source();
        source.rename(SubscriptionName::new("机场 B").unwrap(), 1_700_000_100);
        assert_eq!(source.updated_at(), 1_700_000_100);
        assert_eq!(source.name().value(), "机场 B");
    }

    #[test]
    fn no_op_changes_keep_updated_at() {
        let mut source = sample_source();
        let same_name = source.name().clone();
        source.rename(same_name, 1_700_000_100);
        assert_eq!(source.updated_at(), 1_700_000_000);

        source.set_enabled(true, 1_700_000_100);
        assert_eq!(source.updated_at(), 1_700_000_000);

        let same_url = source.url().clone();
        source.change_url(same_url, 1_700_000_100);
        assert_eq!(source.updated_at(), 1_700_000_000);
    }

    #[test]
    fn restore_keeps_state() {
        let id = SourceId::new();
        let user_id = UserId::new();
        let name = SubscriptionName::new("机场").unwrap();
        let url = SourceUrl::new("https://example.com/sub").unwrap();
        let source = Source::restore(
            id.clone(),
            user_id.clone(),
            name.clone(),
            url.clone(),
            false,
            100,
            200,
        );
        assert_eq!(source.id(), &id);
        assert_eq!(source.user_id(), &user_id);
        assert_eq!(source.name(), &name);
        assert_eq!(source.url(), &url);
        assert!(!source.enabled());
        assert_eq!(source.created_at(), 100);
        assert_eq!(source.updated_at(), 200);
    }
}
