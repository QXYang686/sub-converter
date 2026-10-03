use std::sync::Arc;

use user::UserId;

use crate::domain::RuleSetRepository;

use super::dto::RuleSetView;
use super::error::AppError;

pub struct ListRuleSetsHandler {
    rule_sets: Arc<dyn RuleSetRepository>,
}

impl ListRuleSetsHandler {
    pub fn new(rule_sets: Arc<dyn RuleSetRepository>) -> Self {
        Self { rule_sets }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, user_id: UserId) -> Result<Vec<RuleSetView>, AppError> {
        let rule_sets = self.rule_sets.list_by_user(&user_id).await?;
        let mut views = Vec::with_capacity(rule_sets.len());
        for rule_set in &rule_sets {
            let content = self.rule_sets.find_content(rule_set.id()).await?;
            views.push(RuleSetView::from_parts(rule_set, content.as_ref()));
        }
        Ok(views)
    }
}
