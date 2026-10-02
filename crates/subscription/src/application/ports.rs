use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::SourceUrl;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PortError {
    #[error("secret generation failed: {0}")]
    Failure(String),
}

pub trait Clock: Send + Sync {
    fn now(&self) -> i64;
}

pub trait SecretGenerator: Send + Sync {
    fn generate(&self) -> Result<String, PortError>;
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
    pub subscription_userinfo: Option<String>,
    pub profile_update_interval: Option<String>,
    pub profile_web_page_url: Option<String>,
    pub content_disposition: Option<String>,
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
        url: &SourceUrl,
        validators: &FetchValidators,
    ) -> Result<FetchOutcome, FetchError>;
}

pub type BackgroundTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub trait BackgroundTasks: Send + Sync {
    fn spawn(&self, task: BackgroundTask);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionFormat {
    Clash,
}

impl SubscriptionFormat {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "clash" => Some(Self::Clash),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Clash => "clash",
        }
    }
}
