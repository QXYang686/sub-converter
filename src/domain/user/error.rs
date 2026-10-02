use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid user id")]
    InvalidUserId,
    #[error("invalid username")]
    InvalidUsername,
    #[error("invalid password")]
    InvalidPassword,
    #[error("password hash must not be empty")]
    InvalidPasswordHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepositoryError {
    #[error("username already exists")]
    UsernameConflict,
    #[error("storage unavailable: {0}")]
    Unavailable(String),
}
