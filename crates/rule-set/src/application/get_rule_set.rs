use std::sync::Arc;

use user::UserId;

use crate::domain::{RuleSetId, RuleSetRepository};

use super::dto::RuleSetView;
use super::error::AppError;

#[derive(Debug, Clone)]
pub struct GetRuleSetCommand {
    pub user_id: UserId,
    pub id: String,
}

pub struct GetRuleSetHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
}

impl GetRuleSetHandler {
    pub fn new(rule_sets: Arc<dyn RuleSetRepository>) -> Self {
        Self { rule_sets }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: GetRuleSetCommand) -> Result<RuleSetView, AppError> {
        let id = RuleSetId::parse(&command.id).map_err(|_| AppError::NotFound)?;
        let rule_set = self
            .rule_sets
            .find_by_id(&command.user_id, &id)
            .await?
            .ok_or(AppError::NotFound)?;
        let content = self.rule_sets.find_content(&id).await?;
        Ok(RuleSetView::from_parts(&rule_set, content.as_ref()))
    }
}
