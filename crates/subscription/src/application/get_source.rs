use std::sync::Arc;

use user::UserId;

use crate::domain::{SourceId, SourceRepository};

use super::dto::SourceView;
use super::error::AppError;

#[derive(Debug, Clone)]
pub struct GetSourceCommand {
    pub user_id: UserId,
    pub source_id: String,
}

pub struct GetSourceHandler {
    sources: Arc<dyn SourceRepository>,
}

impl GetSourceHandler {
    pub fn new(sources: Arc<dyn SourceRepository>) -> Self {
        Self { sources }
    }

    pub async fn handle(&self, command: GetSourceCommand) -> Result<SourceView, AppError> {
        let source_id = SourceId::parse(&command.source_id).map_err(|_| AppError::NotFound)?;
        let source = self
            .sources
            .find_by_id(&command.user_id, &source_id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(SourceView::from(&source))
    }
}
