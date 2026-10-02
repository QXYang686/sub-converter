use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid password")]
    InvalidPassword,
    #[error("password hash must not be empty")]
    InvalidPasswordHash,
    #[error("invalid credential id")]
    InvalidCredentialId,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepositoryError {
    #[error("storage unavailable: {0}")]
    Unavailable(String),
}
