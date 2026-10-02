use async_trait::async_trait;
use user::UserId;

use super::{
    Publication, PublicationId, PublicationSecret, RepositoryError, Source, SourceId,
    SourceSnapshot,
};

#[async_trait]
pub trait SourceRepository: Send + Sync {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &SourceId,
    ) -> Result<Option<Source>, RepositoryError>;

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Source>, RepositoryError>;

    async fn save(&self, source: &Source) -> Result<(), RepositoryError>;

    async fn update(&self, source: &Source) -> Result<(), RepositoryError>;

    async fn delete(&self, user_id: &UserId, id: &SourceId) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait PublicationRepository: Send + Sync {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &PublicationId,
    ) -> Result<Option<Publication>, RepositoryError>;

    async fn find_by_secret(
        &self,
        secret: &PublicationSecret,
    ) -> Result<Option<Publication>, RepositoryError>;

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Publication>, RepositoryError>;

    async fn save(&self, publication: &Publication) -> Result<(), RepositoryError>;

    async fn update(&self, publication: &Publication) -> Result<(), RepositoryError>;

    async fn delete(&self, user_id: &UserId, id: &PublicationId) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait SnapshotRepository: Send + Sync {
    async fn find_by_source(
        &self,
        source_id: &SourceId,
    ) -> Result<Option<SourceSnapshot>, RepositoryError>;

    async fn list_by_sources(
        &self,
        source_ids: &[SourceId],
    ) -> Result<Vec<SourceSnapshot>, RepositoryError>;

    async fn save(&self, snapshot: &SourceSnapshot) -> Result<(), RepositoryError>;

    async fn touch(&self, source_id: &SourceId, fetched_at: i64) -> Result<(), RepositoryError>;

    async fn record_error(&self, source_id: &SourceId, error: &str) -> Result<(), RepositoryError>;

    async fn clear(&self, source_id: &SourceId) -> Result<(), RepositoryError>;

    async fn try_acquire_lease(
        &self,
        source_id: &SourceId,
        now: i64,
        lease_until: i64,
    ) -> Result<bool, RepositoryError>;
}
