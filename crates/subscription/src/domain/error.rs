use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid source id")]
    InvalidSourceId,
    #[error("invalid publication id")]
    InvalidPublicationId,
    #[error("invalid subscription name")]
    InvalidSubscriptionName,
    #[error("invalid source url")]
    InvalidSourceUrl,
    #[error("invalid publication secret")]
    InvalidPublicationSecret,
    #[error("duplicate source in publication")]
    DuplicateSource,
    #[error("too many sources in publication")]
    TooManySources,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepositoryError {
    #[error("source url already exists")]
    SourceUrlConflict,
    #[error("publication secret already exists")]
    SecretConflict,
    #[error("storage unavailable: {0}")]
    Unavailable(String),
}
