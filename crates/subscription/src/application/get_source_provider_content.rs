use std::sync::Arc;

use user::UserId;

use crate::domain::{RuleProviderRepository, SourceId, SourceRepository};

use super::error::AppError;

#[derive(Debug, Clone)]
pub struct GetSourceProviderContentCommand {
    pub user_id: UserId,
    pub source_id: String,
    pub name: String,
}

/// 返回服务端为某个 rule-provider 抓取的最近一次原始内容。
pub struct GetSourceProviderContentHandler {
    sources: Arc<dyn SourceRepository>,
    rule_providers: Arc<dyn RuleProviderRepository>,
}

impl GetSourceProviderContentHandler {
    pub fn new(
        sources: Arc<dyn SourceRepository>,
        rule_providers: Arc<dyn RuleProviderRepository>,
    ) -> Self {
        Self {
            sources,
            rule_providers,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: GetSourceProviderContentCommand,
    ) -> Result<String, AppError> {
        let source_id = SourceId::parse(&command.source_id).map_err(|_| AppError::NotFound)?;
        self.sources
            .find_by_id(&command.user_id, &source_id)
            .await?
            .ok_or(AppError::NotFound)?;

        let snapshot = self
            .rule_providers
            .find_snapshot(&source_id, &command.name)
            .await?
            .ok_or(AppError::NotFound)?;
        if snapshot.body().is_empty() {
            return Err(AppError::NotFound);
        }
        Ok(String::from_utf8_lossy(snapshot.body()).into_owned())
    }
}
