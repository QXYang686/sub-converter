use std::collections::HashMap;
use std::sync::Arc;

use user::UserId;

use crate::domain::{
    ExtractedRuleProvider, RuleProviderRepository, RuleProviderSnapshot, SourceId, SourceRepository,
};

use super::dto::RuleProviderView;
use super::error::AppError;

#[derive(Debug, Clone)]
pub struct ListSourceProvidersCommand {
    pub user_id: UserId,
    pub source_id: String,
}

/// 列出某订阅源声明的 rule-provider 及其服务端抓取快照摘要。
pub struct ListSourceProvidersHandler {
    sources: Arc<dyn SourceRepository>,
    rule_providers: Arc<dyn RuleProviderRepository>,
}

impl ListSourceProvidersHandler {
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
        command: ListSourceProvidersCommand,
    ) -> Result<Vec<RuleProviderView>, AppError> {
        let source_id = SourceId::parse(&command.source_id).map_err(|_| AppError::NotFound)?;
        self.sources
            .find_by_id(&command.user_id, &source_id)
            .await?
            .ok_or(AppError::NotFound)?;

        let providers = self.rule_providers.list_providers(&source_id).await?;
        let snapshots = self.rule_providers.list_snapshots(&source_id).await?;
        let by_name: HashMap<&str, &RuleProviderSnapshot> = snapshots
            .iter()
            .map(|snapshot| (snapshot.name(), snapshot))
            .collect();

        Ok(providers
            .into_iter()
            .map(|provider| view(&provider, by_name.get(provider.name.as_str()).copied()))
            .collect())
    }
}

fn view(
    provider: &ExtractedRuleProvider,
    snapshot: Option<&RuleProviderSnapshot>,
) -> RuleProviderView {
    RuleProviderView {
        name: provider.name.clone(),
        provider_type: provider.provider_type.clone(),
        behavior: provider.behavior.clone(),
        url: provider.url.clone(),
        interval: provider.interval,
        rule_count: snapshot.map(RuleProviderSnapshot::rule_count).unwrap_or(0),
        fetched_at: snapshot
            .filter(|snapshot| snapshot.fetched_at() > 0)
            .map(RuleProviderSnapshot::fetched_at),
        has_snapshot: snapshot.is_some_and(|snapshot| !snapshot.body().is_empty()),
        last_error: snapshot.and_then(|snapshot| snapshot.last_error().map(str::to_string)),
    }
}
