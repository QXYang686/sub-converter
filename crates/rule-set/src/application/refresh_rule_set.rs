use std::sync::Arc;

use user::UserId;

use crate::domain::{count_rules, RuleSet, RuleSetContent, RuleSetId, RuleSetRepository};

use super::error::AppError;
use super::ports::{Clock, FetchOutcome, FetchValidators, Fetcher};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshStatus {
    /// 抓到新内容并写入。
    Refreshed,
    /// 上游 304，只更新时间。
    NotModified,
    /// 非 remote、已禁用或已被手动钉住，跳过。
    Skipped,
    /// 抓取失败，保留旧内容并记录错误。
    Failed,
}

impl RefreshStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Refreshed => "refreshed",
            Self::NotModified => "notModified",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RefreshRuleSetCommand {
    pub user_id: UserId,
    pub id: String,
}

pub struct RefreshRuleSetHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
    fetcher: Arc<dyn Fetcher>,
    clock: Arc<dyn Clock>,
}

impl RefreshRuleSetHandler {
    pub fn new(
        rule_sets: Arc<dyn RuleSetRepository>,
        fetcher: Arc<dyn Fetcher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            rule_sets,
            fetcher,
            clock,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: RefreshRuleSetCommand,
    ) -> Result<RefreshStatus, AppError> {
        let id = RuleSetId::parse(&command.id).map_err(|_| AppError::NotFound)?;
        let rule_set = self
            .rule_sets
            .find_by_id(&command.user_id, &id)
            .await?
            .ok_or(AppError::NotFound)?;
        fetch_into_content(
            self.rule_sets.as_ref(),
            self.fetcher.as_ref(),
            &rule_set,
            self.clock.now(),
        )
        .await
    }
}

/// 对 `Remote` 规则集做条件请求抓取并写入内容。创建与手动刷新共用。
pub(super) async fn fetch_into_content(
    rule_sets: &dyn RuleSetRepository,
    fetcher: &dyn Fetcher,
    rule_set: &RuleSet,
    now: i64,
) -> Result<RefreshStatus, AppError> {
    let Some(url) = rule_set.source().url() else {
        return Ok(RefreshStatus::Skipped);
    };
    if !rule_set.enabled() {
        return Ok(RefreshStatus::Skipped);
    }

    let previous = rule_sets.find_content(rule_set.id()).await?;
    if previous.as_ref().is_some_and(|content| content.pinned()) {
        return Ok(RefreshStatus::Skipped);
    }

    let validators = previous
        .as_ref()
        .map(|content| FetchValidators {
            etag: content.etag().map(str::to_string),
            last_modified: content.last_modified().map(str::to_string),
        })
        .unwrap_or_default();
    let mut content = previous.unwrap_or_else(|| RuleSetContent::empty(rule_set.id().clone()));

    match fetcher.fetch(url, &validators).await {
        Ok(FetchOutcome::Fetched(document)) => {
            let rule_count = count_rules(rule_set.content_format(), &document.body);
            let etag = document.etag;
            let last_modified = document.last_modified;
            content.set_body(document.body, rule_count, now);
            content.set_validators(etag, last_modified);
            rule_sets.save_content(&content).await?;
            Ok(RefreshStatus::Refreshed)
        }
        Ok(FetchOutcome::NotModified) => {
            content.mark_refreshed(now);
            rule_sets.save_content(&content).await?;
            Ok(RefreshStatus::NotModified)
        }
        Err(err) => {
            content.set_error(&err.to_string());
            rule_sets.save_content(&content).await?;
            Ok(RefreshStatus::Failed)
        }
    }
}
