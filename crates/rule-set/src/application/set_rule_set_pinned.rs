use std::sync::Arc;

use user::UserId;

use crate::domain::{RuleSetContent, RuleSetId, RuleSetRepository, RuleSetSource};

use super::dto::RuleSetView;
use super::error::AppError;

#[derive(Debug, Clone)]
pub struct SetRuleSetPinnedCommand {
    pub user_id: UserId,
    pub id: String,
    pub pinned: bool,
}

pub struct SetRuleSetPinnedHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
}

impl SetRuleSetPinnedHandler {
    pub fn new(rule_sets: Arc<dyn RuleSetRepository>) -> Self {
        Self { rule_sets }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: SetRuleSetPinnedCommand,
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

        let mut content = self
            .rule_sets
            .find_content(&id)
            .await?
            .unwrap_or_else(|| RuleSetContent::empty(id.clone()));
        content.set_pinned(command.pinned);
        self.rule_sets.save_content(&content).await?;

        Ok(RuleSetView::from_parts(&rule_set, Some(&content)))
    }
}
