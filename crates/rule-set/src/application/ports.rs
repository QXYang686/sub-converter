use async_trait::async_trait;
use thiserror::Error;

use crate::domain::RuleSetUrl;

pub trait Clock: Send + Sync {
    fn now(&self) -> i64;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchValidators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedDocument {
    pub body: Vec<u8>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    Fetched(FetchedDocument),
    NotModified,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum FetchError {
    #[error("request failed: {0}")]
    Failure(String),
    #[error("unexpected upstream status {0}")]
    Status(u16),
    #[error("upstream response exceeds size limit")]
    TooLarge,
}

#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn fetch(
        &self,
        url: &RuleSetUrl,
        validators: &FetchValidators,
    ) -> Result<FetchOutcome, FetchError>;
}
