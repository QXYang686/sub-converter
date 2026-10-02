use user::UserId;

use super::{SourceId, SourceUrl, SubscriptionName};

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
