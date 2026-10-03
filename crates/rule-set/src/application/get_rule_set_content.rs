use std::sync::Arc;

use user::UserId;

use crate::domain::{RuleSetId, RuleSetRepository};

use super::error::AppError;

#[derive(Debug, Clone)]
pub struct GetRuleSetContentCommand {
    pub user_id: UserId,
    pub id: String,
}

pub struct GetRuleSetContentHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
}

impl GetRuleSetContentHandler {
    pub fn new(rule_sets: Arc<dyn RuleSetRepository>) -> Self {
        Self { rule_sets }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: GetRuleSetContentCommand) -> Result<Vec<u8>, AppError> {
        let id = RuleSetId::parse(&command.id).map_err(|_| AppError::NotFound)?;
        self.rule_sets
            .find_by_id(&command.user_id, &id)
            .await?
            .ok_or(AppError::NotFound)?;
        let content = self
            .rule_sets
            .find_content(&id)
            .await?
            .ok_or(AppError::NotFound)?;
        content
            .body()
            .filter(|body| !body.is_empty())
            .map(<[u8]>::to_vec)
            .ok_or(AppError::NotFound)
    }
}
