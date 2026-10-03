use std::sync::Arc;

use user::UserId;

use crate::domain::{
    count_rules, RuleSetId, RuleSetInterval, RuleSetName, RuleSetPath, RuleSetRepository,
    RuleSetSource, RuleSetUrl,
};

use super::dto::RuleSetView;
use super::error::AppError;
use super::parse::{map_repository_error, parse_category, parse_content_format};
use super::ports::{Clock, Fetcher};
use super::refresh_rule_set::fetch_into_content;

#[derive(Debug, Clone)]
pub struct UpdateRuleSetCommand {
    pub user_id: UserId,
    pub id: String,
    pub name: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    /// 外层 `None` 表示不改，内层 `None` 表示清空分类。
    pub category: Option<Option<String>>,
    pub content_format: Option<String>,
    /// 外层 `None` 表示不改，内层 `None` 表示清空间隔。
    pub interval: Option<Option<i64>>,
    pub enabled: Option<bool>,
}

pub struct UpdateRuleSetHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
    fetcher: Arc<dyn Fetcher>,
    clock: Arc<dyn Clock>,
}

impl UpdateRuleSetHandler {
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
    pub async fn handle(&self, command: UpdateRuleSetCommand) -> Result<RuleSetView, AppError> {
        let id = RuleSetId::parse(&command.id).map_err(|_| AppError::NotFound)?;
        let mut rule_set = self
            .rule_sets
            .find_by_id(&command.user_id, &id)
            .await?
            .ok_or(AppError::NotFound)?;
        let now = self.clock.now();

        if let Some(name) = command.name {
            rule_set.rename(RuleSetName::new(&name)?, now);
        }
        let mut url_changed = false;
        if let Some(url) = command.url {
            if !matches!(rule_set.source(), RuleSetSource::Remote(_)) {
                return Err(AppError::NotRemote);
            }
            rule_set.change_source(RuleSetSource::Remote(RuleSetUrl::new(&url)?), now);
            url_changed = true;
        }
        if let Some(path) = command.path {
            if !matches!(rule_set.source(), RuleSetSource::Local(_)) {
                return Err(AppError::InvalidSourceKind);
            }
            rule_set.change_source(RuleSetSource::Local(RuleSetPath::new(&path)?), now);
        }
        if let Some(category) = command.category {
            rule_set.set_category(parse_category(category.as_deref())?, now);
        }
        if let Some(format) = command.content_format {
            rule_set.set_content_format(parse_content_format(&format)?, now);
        }
        if let Some(interval) = command.interval {
            rule_set.set_interval(interval.map(RuleSetInterval::new).transpose()?, now);
        }
        if let Some(enabled) = command.enabled {
            rule_set.set_enabled(enabled, now);
        }

        self.rule_sets
            .update(&rule_set)
            .await
            .map_err(map_repository_error)?;

        self.recount_content(&rule_set).await?;
        if url_changed && rule_set.is_remote() {
            fetch_into_content(
                self.rule_sets.as_ref(),
                self.fetcher.as_ref(),
                &rule_set,
                now,
            )
            .await?;
        }

        let content = self.rule_sets.find_content(rule_set.id()).await?;
        Ok(RuleSetView::from_parts(&rule_set, content.as_ref()))
    }

    /// 内容格式可能变化，重新按当前格式统计规则条数。
    async fn recount_content(&self, rule_set: &crate::domain::RuleSet) -> Result<(), AppError> {
        let Some(mut content) = self.rule_sets.find_content(rule_set.id()).await? else {
            return Ok(());
        };
        let Some(body) = content.body().map(<[u8]>::to_vec) else {
            return Ok(());
        };
        let rule_count = count_rules(rule_set.content_format(), &body);
        if rule_count != content.rule_count() {
            let updated_at = content.updated_at();
            content.set_body(body, rule_count, updated_at);
            self.rule_sets.save_content(&content).await?;
        }
        Ok(())
    }
}
