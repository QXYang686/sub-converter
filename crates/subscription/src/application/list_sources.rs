use std::collections::HashMap;
use std::sync::Arc;

use user::UserId;

use crate::domain::{SnapshotRepository, SourceId, SourceRepository, SourceSnapshot};

use super::dto::SourceView;
use super::error::AppError;

pub struct ListSourcesHandler {
    sources: Arc<dyn SourceRepository>,
    snapshots: Arc<dyn SnapshotRepository>,
}

impl ListSourcesHandler {
    pub fn new(sources: Arc<dyn SourceRepository>, snapshots: Arc<dyn SnapshotRepository>) -> Self {
        Self { sources, snapshots }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, user_id: &UserId) -> Result<Vec<SourceView>, AppError> {
        let sources = self.sources.list_by_user(user_id).await?;
        let ids: Vec<SourceId> = sources.iter().map(|source| source.id().clone()).collect();
        let snapshots = self.snapshots.list_by_sources(&ids).await?;
        let by_source: HashMap<&SourceId, &SourceSnapshot> = snapshots
            .iter()
            .map(|snapshot| (snapshot.source_id(), snapshot))
            .collect();

        Ok(sources
            .iter()
            .map(|source| SourceView::with_snapshot(source, by_source.get(source.id()).copied()))
            .collect())
    }
}
