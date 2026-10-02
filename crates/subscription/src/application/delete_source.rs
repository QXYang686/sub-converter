use std::sync::Arc;

use user::UserId;

use crate::domain::{SourceId, SourceRepository};

use super::error::AppError;

#[derive(Debug, Clone)]
pub struct DeleteSourceCommand {
    pub user_id: UserId,
    pub source_id: String,
}

pub struct DeleteSourceHandler {
    sources: Arc<dyn SourceRepository>,
}

impl DeleteSourceHandler {
    pub fn new(sources: Arc<dyn SourceRepository>) -> Self {
        Self { sources }
    }

    pub async fn handle(&self, command: DeleteSourceCommand) -> Result<(), AppError> {
        let source_id = SourceId::parse(&command.source_id).map_err(|_| AppError::NotFound)?;
        if self
            .sources
            .find_by_id(&command.user_id, &source_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound);
        }
        self.sources.delete(&command.user_id, &source_id).await?;
        Ok(())
    }
}
