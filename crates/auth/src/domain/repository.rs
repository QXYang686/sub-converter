use async_trait::async_trait;
use user::UserId;

use super::{Challenge, Credential, CredentialId, RepositoryError};

#[async_trait]
pub trait CredentialRepository: Send + Sync {
    async fn find_password_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<Credential>, RepositoryError>;

    async fn find_passkeys_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<Credential>, RepositoryError>;

    async fn find_passkey_by_credential_id(
        &self,
        credential_id: &str,
    ) -> Result<Option<Credential>, RepositoryError>;

    async fn save(&self, credential: &Credential) -> Result<(), RepositoryError>;

    async fn mark_used(&self, id: &CredentialId, now: i64) -> Result<(), RepositoryError>;

    async fn update_sign_count(
        &self,
        id: &CredentialId,
        sign_count: u32,
    ) -> Result<(), RepositoryError>;

    async fn delete(&self, id: &CredentialId) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait ChallengeRepository: Send + Sync {
    async fn save(&self, challenge: &Challenge) -> Result<(), RepositoryError>;

    async fn consume(&self, challenge: &str) -> Result<Option<Challenge>, RepositoryError>;
}
