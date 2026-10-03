use std::sync::Arc;

use user::UserId;

use crate::domain::{
    count_rules, RuleSet, RuleSetContent, RuleSetId, RuleSetInterval, RuleSetName,
    RuleSetRepository, RuleSetSource,
};

use super::dto::RuleSetView;
use super::error::AppError;
use super::parse::{map_repository_error, parse_category, parse_content_format, parse_source};
use super::ports::{Clock, Fetcher};
use super::refresh_rule_set::fetch_into_content;

#[derive(Debug, Clone)]
pub struct CreateRuleSetCommand {
    pub user_id: UserId,
    pub name: String,
    pub source_kind: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub category: Option<String>,
    pub content_format: String,
    pub interval: Option<i64>,
    /// `Inline` 来源的初始内容。
    pub content: Option<String>,
}

pub struct CreateRuleSetHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
    fetcher: Arc<dyn Fetcher>,
    clock: Arc<dyn Clock>,
}

impl CreateRuleSetHandler {
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
    pub async fn handle(&self, command: CreateRuleSetCommand) -> Result<RuleSetView, AppError> {
        let name = RuleSetName::new(&command.name)?;
        let category = parse_category(command.category.as_deref())?;
        let content_format = parse_content_format(&command.content_format)?;
        let interval = command.interval.map(RuleSetInterval::new).transpose()?;
        let source = parse_source(
            &command.source_kind,
            command.url.as_deref(),
            command.path.as_deref(),
        )?;
        let now = self.clock.now();

        let rule_set = RuleSet::create(
            RuleSetId::new(),
            command.user_id,
            name,
            source,
            category,
            content_format,
            interval,
            now,
        );
        self.rule_sets
            .save(&rule_set)
            .await
            .map_err(map_repository_error)?;

        match rule_set.source() {
            RuleSetSource::Inline => {
                let text = command.content.ok_or(AppError::MissingContent)?;
                let body = text.into_bytes();
                let rule_count = count_rules(content_format, &body);
                let mut content = RuleSetContent::empty(rule_set.id().clone());
                content.set_body(body, rule_count, now);
                content.set_pinned(true);
                self.rule_sets.save_content(&content).await?;
            }
            RuleSetSource::Remote(_) => {
                fetch_into_content(
                    self.rule_sets.as_ref(),
                    self.fetcher.as_ref(),
                    &rule_set,
                    now,
                )
                .await?;
            }
            RuleSetSource::Local(_) => {}
        }

        let content = self.rule_sets.find_content(rule_set.id()).await?;
        Ok(RuleSetView::from_parts(&rule_set, content.as_ref()))
    }
}
