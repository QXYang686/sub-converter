use thiserror::Error;

use crate::domain::{DomainError, RepositoryError};

use super::ports::FetchError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("invalid rule set id")]
    InvalidId,
    #[error("invalid rule set name")]
    InvalidName,
    #[error("invalid rule set url")]
    InvalidUrl,
    #[error("invalid rule set path")]
    InvalidPath,
    #[error("invalid rule set interval")]
    InvalidInterval,
    #[error("invalid rule set category")]
    InvalidCategory,
    #[error("invalid rule set content format")]
    InvalidContentFormat,
    #[error("invalid rule set source kind")]
    InvalidSourceKind,
    #[error("rule set name already exists")]
    NameTaken,
    #[error("rule set source is not remote")]
    NotRemote,
    #[error("rule set content is not editable")]
    ContentNotEditable,
    #[error("missing rule set content")]
    MissingContent,
    #[error("not found")]
    NotFound,
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<DomainError> for AppError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::InvalidRuleSetId => AppError::InvalidId,
            DomainError::InvalidRuleSetName => AppError::InvalidName,
            DomainError::InvalidRuleSetUrl => AppError::InvalidUrl,
            DomainError::InvalidRuleSetPath => AppError::InvalidPath,
            DomainError::InvalidRuleSetInterval => AppError::InvalidInterval,
        }
    }
}
