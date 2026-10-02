use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::PasswordHash;
use user::UserId;

#[derive(Debug, Error)]
pub enum PortError {
    #[error("invalid token")]
    InvalidToken,
    #[error("port failure: {0}")]
    Failure(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessToken {
    pub token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRefreshToken {
    pub id: Uuid,
    pub user_id: UserId,
    pub token_hash: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedRefreshToken {
    pub raw: String,
    pub record: StoredRefreshToken,
}

pub trait Clock: Send + Sync {
    fn now(&self) -> i64;
}

#[async_trait]
pub trait PasswordHasher: Send + Sync {
    async fn hash(&self, password: &str) -> Result<PasswordHash, PortError>;

    async fn verify(&self, password: &str, hash: &PasswordHash) -> Result<bool, PortError>;
}

#[async_trait]
pub trait TokenService: Send + Sync {
    async fn issue_access_token(
        &self,
        user_id: &UserId,
        now: i64,
    ) -> Result<AccessToken, PortError>;

    async fn verify_access_token(&self, token: &str, now: i64) -> Result<UserId, PortError>;

    async fn issue_refresh_token(
        &self,
        user_id: &UserId,
        now: i64,
    ) -> Result<IssuedRefreshToken, PortError>;

    fn hash_refresh_token(&self, raw: &str) -> String;
}

#[async_trait]
pub trait RefreshTokenRepository: Send + Sync {
    async fn save(&self, token: &StoredRefreshToken) -> Result<(), PortError>;

    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<StoredRefreshToken>, PortError>;

    async fn revoke(&self, id: Uuid, now: i64) -> Result<(), PortError>;

    async fn revoke_all_for_user(&self, user_id: &UserId, now: i64) -> Result<(), PortError>;
}
