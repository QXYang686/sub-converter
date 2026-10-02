use std::collections::HashSet;

use user::UserId;

use super::{
    DomainError, PublicationId, PublicationSecret, SourceId, SubscriptionName,
    PUBLICATION_MAX_SOURCES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationSource {
    source_id: SourceId,
    position: u32,
}

impl PublicationSource {
    pub fn new(source_id: SourceId, position: u32) -> Self {
        Self {
            source_id,
            position,
        }
    }

    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    pub fn position(&self) -> u32 {
        self.position
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publication {
    id: PublicationId,
    user_id: UserId,
    name: SubscriptionName,
    secret: PublicationSecret,
    enabled: bool,
    expires_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
    sources: Vec<PublicationSource>,
}

impl Publication {
    pub fn create(
        id: PublicationId,
        user_id: UserId,
        name: SubscriptionName,
        secret: PublicationSecret,
        now: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            name,
            secret,
            enabled: true,
            expires_at: None,
            created_at: now,
            updated_at: now,
            sources: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: PublicationId,
        user_id: UserId,
        name: SubscriptionName,
        secret: PublicationSecret,
        enabled: bool,
        expires_at: Option<i64>,
        created_at: i64,
        updated_at: i64,
        sources: Vec<PublicationSource>,
    ) -> Self {
        Self {
            id,
            user_id,
            name,
            secret,
            enabled,
            expires_at,
            created_at,
            updated_at,
            sources,
        }
    }

    pub fn rename(&mut self, name: SubscriptionName, now: i64) {
        if self.name != name {
            self.name = name;
            self.updated_at = now;
        }
    }

    pub fn set_enabled(&mut self, enabled: bool, now: i64) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.updated_at = now;
        }
    }

    pub fn set_expires_at(&mut self, expires_at: Option<i64>, now: i64) {
        if self.expires_at != expires_at {
            self.expires_at = expires_at;
            self.updated_at = now;
        }
    }

    pub fn replace_sources(
        &mut self,
        source_ids: Vec<SourceId>,
        now: i64,
    ) -> Result<(), DomainError> {
        if source_ids.len() > PUBLICATION_MAX_SOURCES {
            return Err(DomainError::TooManySources);
        }
        let mut seen: HashSet<&SourceId> = HashSet::with_capacity(source_ids.len());
        for source_id in &source_ids {
            if !seen.insert(source_id) {
                return Err(DomainError::DuplicateSource);
            }
        }

        self.sources = source_ids
            .into_iter()
            .enumerate()
            .map(|(position, source_id)| PublicationSource::new(source_id, position as u32))
            .collect();
        self.updated_at = now;
        Ok(())
    }

    pub fn id(&self) -> &PublicationId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn name(&self) -> &SubscriptionName {
        &self.name
    }

    pub fn secret(&self) -> &PublicationSecret {
        &self.secret
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn expires_at(&self) -> Option<i64> {
        self.expires_at
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    pub fn sources(&self) -> &[PublicationSource] {
        &self.sources
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_publication() -> Publication {
        Publication::create(
            PublicationId::new(),
            UserId::new(),
            SubscriptionName::new("我的订阅").unwrap(),
            PublicationSecret::new(&"a".repeat(32)).unwrap(),
            1_700_000_000,
        )
    }

    #[test]
    fn create_enables_without_expiry_or_sources() {
        let publication = sample_publication();
        assert!(publication.enabled());
        assert_eq!(publication.expires_at(), None);
        assert!(publication.sources().is_empty());
    }

    #[test]
    fn replace_sources_assigns_positions_in_order() {
        let mut publication = sample_publication();
        let first = SourceId::new();
        let second = SourceId::new();
        publication
            .replace_sources(vec![first.clone(), second.clone()], 1_700_000_100)
            .unwrap();

        assert_eq!(publication.updated_at(), 1_700_000_100);
        let sources = publication.sources();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].source_id(), &first);
        assert_eq!(sources[0].position(), 0);
        assert_eq!(sources[1].source_id(), &second);
        assert_eq!(sources[1].position(), 1);
    }

    #[test]
    fn replace_sources_rejects_duplicates() {
        let mut publication = sample_publication();
        let id = SourceId::new();
        let result = publication.replace_sources(vec![id.clone(), id], 1_700_000_100);
        assert_eq!(result, Err(DomainError::DuplicateSource));
    }

    #[test]
    fn replace_sources_rejects_too_many() {
        let mut publication = sample_publication();
        let ids = (0..=PUBLICATION_MAX_SOURCES)
            .map(|_| SourceId::new())
            .collect();
        let result = publication.replace_sources(ids, 1_700_000_100);
        assert_eq!(result, Err(DomainError::TooManySources));
    }

    #[test]
    fn expiry_can_be_set_and_cleared() {
        let mut publication = sample_publication();
        publication.set_expires_at(Some(1_800_000_000), 1_700_000_100);
        assert_eq!(publication.expires_at(), Some(1_800_000_000));
        assert_eq!(publication.updated_at(), 1_700_000_100);

        publication.set_expires_at(None, 1_700_000_200);
        assert_eq!(publication.expires_at(), None);
        assert_eq!(publication.updated_at(), 1_700_000_200);
    }
}
