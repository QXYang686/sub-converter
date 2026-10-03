use async_trait::async_trait;
use user::UserId;

use super::{RepositoryError, RuleSet, RuleSetContent, RuleSetId};

#[async_trait]
pub trait RuleSetRepository: Send + Sync {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &RuleSetId,
    ) -> Result<Option<RuleSet>, RepositoryError>;

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<RuleSet>, RepositoryError>;

    async fn save(&self, rule_set: &RuleSet) -> Result<(), RepositoryError>;

    async fn update(&self, rule_set: &RuleSet) -> Result<(), RepositoryError>;

    async fn delete(&self, user_id: &UserId, id: &RuleSetId) -> Result<(), RepositoryError>;

    async fn find_content(
        &self,
        id: &RuleSetId,
    ) -> Result<Option<RuleSetContent>, RepositoryError>;

    async fn save_content(&self, content: &RuleSetContent) -> Result<(), RepositoryError>;

    async fn delete_content(&self, id: &RuleSetId) -> Result<(), RepositoryError>;
}
