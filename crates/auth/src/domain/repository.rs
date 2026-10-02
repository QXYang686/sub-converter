use async_trait::async_trait;
use user::UserId;

use super::{CredentialId, PasswordCredential, RepositoryError};

#[async_trait]
pub trait PasswordCredentialRepository: Send + Sync {
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<PasswordCredential>, RepositoryError>;

    async fn save(&self, credential: &PasswordCredential) -> Result<(), RepositoryError>;

    async fn mark_used(&self, id: &CredentialId, now: i64) -> Result<(), RepositoryError>;
}
