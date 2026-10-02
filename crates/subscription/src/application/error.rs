use thiserror::Error;

use crate::domain::RepositoryError;

use super::ports::PortError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Port(#[from] PortError),
    #[error("invalid subscription name")]
    InvalidName,
    #[error("invalid source url")]
    InvalidSourceUrl,
    #[error("invalid source id")]
    InvalidSourceId,
    #[error("invalid publication secret")]
    InvalidSecret,
    #[error("source url already exists")]
    SourceUrlTaken,
    #[error("unknown source")]
    UnknownSource,
    #[error("duplicate source in publication")]
    DuplicateSource,
    #[error("too many sources in publication")]
    TooManySources,
    #[error("not found")]
    NotFound,
    #[error("internal error: {0}")]
    Internal(String),
}
