use thiserror::Error;

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
