use thiserror::Error;

use super::ports::PortError;
use crate::domain::RepositoryError as CredentialRepositoryError;
use user::RepositoryError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Credential(#[from] CredentialRepositoryError),
    #[error(transparent)]
    Port(#[from] PortError),
    #[error("invalid username")]
    InvalidUsername,
    #[error("invalid password")]
    InvalidPassword,
    #[error("username already exists")]
    UsernameTaken,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("invalid token")]
    InvalidToken,
    #[error("passkey ceremony failed: {0}")]
    Passkey(String),
    #[error("not found")]
    NotFound,
    #[error("internal error: {0}")]
    Internal(String),
}
