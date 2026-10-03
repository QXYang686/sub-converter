use std::sync::Arc;

use user::UserId;

use crate::domain::{RuleSetId, RuleSetRepository};

use super::error::AppError;

#[derive(Debug, Clone)]
pub struct DeleteRuleSetCommand {
    pub user_id: UserId,
    pub id: String,
}

pub struct DeleteRuleSetHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
}

impl DeleteRuleSetHandler {
    pub fn new(rule_sets: Arc<dyn RuleSetRepository>) -> Self {
        Self { rule_sets }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: DeleteRuleSetCommand) -> Result<(), AppError> {
        let id = RuleSetId::parse(&command.id).map_err(|_| AppError::NotFound)?;
        self.rule_sets
            .find_by_id(&command.user_id, &id)
            .await?
            .ok_or(AppError::NotFound)?;
        self.rule_sets.delete_content(&id).await?;
        self.rule_sets.delete(&command.user_id, &id).await?;
        Ok(())
    }
}
