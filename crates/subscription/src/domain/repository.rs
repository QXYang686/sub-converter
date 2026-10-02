use async_trait::async_trait;
use user::UserId;

use super::{Publication, PublicationId, RepositoryError, Source, SourceId};

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

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Publication>, RepositoryError>;

    async fn save(&self, publication: &Publication) -> Result<(), RepositoryError>;

    async fn update(&self, publication: &Publication) -> Result<(), RepositoryError>;

    async fn delete(&self, user_id: &UserId, id: &PublicationId) -> Result<(), RepositoryError>;
}
