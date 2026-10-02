use async_trait::async_trait;

use super::{RepositoryError, User, UserId, Username};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_username(
        &self,
        username: &Username,
    ) -> Result<Option<User>, RepositoryError>;

    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, RepositoryError>;

    async fn save(&self, user: &User) -> Result<(), RepositoryError>;

    async fn delete(&self, id: &UserId) -> Result<(), RepositoryError>;
}
