use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid rule set id")]
    InvalidRuleSetId,
    #[error("invalid rule set name")]
    InvalidRuleSetName,
    #[error("invalid rule set url")]
    InvalidRuleSetUrl,
    #[error("invalid rule set path")]
    InvalidRuleSetPath,
    #[error("invalid rule set interval")]
    InvalidRuleSetInterval,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepositoryError {
    #[error("rule set name already exists")]
    NameConflict,
    #[error("storage unavailable: {0}")]
    Unavailable(String),
}
