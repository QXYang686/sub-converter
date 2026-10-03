use std::sync::Arc;

use user::UserId;

use crate::domain::{
    count_rules, RuleSetContent, RuleSetId, RuleSetRepository, RuleSetSource,
};

use super::dto::RuleSetView;
use super::error::AppError;
use super::ports::Clock;

#[derive(Debug, Clone)]
pub struct ReplaceRuleSetContentCommand {
    pub user_id: UserId,
    pub id: String,
    pub body: Vec<u8>,
    /// 手动写入的内容是否钉住（钉住后远程刷新不覆盖）。
    pub pin: bool,
}

pub struct ReplaceRuleSetContentHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
    clock: Arc<dyn Clock>,
}

impl ReplaceRuleSetContentHandler {
    pub fn new(rule_sets: Arc<dyn RuleSetRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { rule_sets, clock }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: ReplaceRuleSetContentCommand,
    ) -> Result<RuleSetView, AppError> {
        let id = RuleSetId::parse(&command.id).map_err(|_| AppError::NotFound)?;
        let rule_set = self
            .rule_sets
            .find_by_id(&command.user_id, &id)
            .await?
            .ok_or(AppError::NotFound)?;
        if matches!(rule_set.source(), RuleSetSource::Local(_)) {
            return Err(AppError::ContentNotEditable);
        }

        let now = self.clock.now();
        let mut content = self
            .rule_sets
            .find_content(&id)
            .await?
            .unwrap_or_else(|| RuleSetContent::empty(id.clone()));
        let rule_count = count_rules(rule_set.content_format(), &command.body);
        content.set_body(command.body, rule_count, now);
        content.set_pinned(command.pin);
        self.rule_sets.save_content(&content).await?;

        Ok(RuleSetView::from_parts(&rule_set, Some(&content)))
    }
}
