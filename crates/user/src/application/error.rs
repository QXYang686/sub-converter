use thiserror::Error;

use super::ports::PortError;
use crate::domain::{DomainError, RepositoryError};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Port(#[from] PortError),
    #[error("username already exists")]
    UsernameTaken,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("invalid token")]
    InvalidToken,
    #[error("internal error: {0}")]
    Internal(String),
}
