use std::sync::Arc;

use user::UserId;

use crate::domain::SourceRepository;

use super::dto::SourceView;
use super::error::AppError;

pub struct ListSourcesHandler {
    sources: Arc<dyn SourceRepository>,
}

impl ListSourcesHandler {
    pub fn new(sources: Arc<dyn SourceRepository>) -> Self {
        Self { sources }
    }

    pub async fn handle(&self, user_id: &UserId) -> Result<Vec<SourceView>, AppError> {
        let sources = self.sources.list_by_user(user_id).await?;
        Ok(sources.iter().map(SourceView::from).collect())
    }
}
