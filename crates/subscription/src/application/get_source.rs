use std::sync::Arc;

use user::UserId;

use crate::domain::{SnapshotRepository, SourceId, SourceRepository};

use super::dto::SourceView;
use super::error::AppError;

#[derive(Debug, Clone)]
pub struct GetSourceCommand {
    pub user_id: UserId,
    pub source_id: String,
}

pub struct GetSourceHandler {
    sources: Arc<dyn SourceRepository>,
    snapshots: Arc<dyn SnapshotRepository>,
}

impl GetSourceHandler {
    pub fn new(sources: Arc<dyn SourceRepository>, snapshots: Arc<dyn SnapshotRepository>) -> Self {
        Self { sources, snapshots }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: GetSourceCommand) -> Result<SourceView, AppError> {
        let source_id = SourceId::parse(&command.source_id).map_err(|_| AppError::NotFound)?;
        let source = self
            .sources
            .find_by_id(&command.user_id, &source_id)
            .await?
            .ok_or(AppError::NotFound)?;
        let snapshot = self.snapshots.find_by_source(&source_id).await?;
        Ok(SourceView::with_snapshot(&source, snapshot.as_ref()))
    }
}
