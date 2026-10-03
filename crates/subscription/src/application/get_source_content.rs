use std::sync::Arc;

use user::UserId;

use crate::domain::{SnapshotRepository, SourceId, SourceRepository};

use super::error::AppError;

#[derive(Debug, Clone)]
pub struct GetSourceContentCommand {
    pub user_id: UserId,
    pub source_id: String,
}

/// 返回订阅源最近一次成功抓取的原始 body，供管理端以文本形式查看。
pub struct GetSourceContentHandler {
    sources: Arc<dyn SourceRepository>,
    snapshots: Arc<dyn SnapshotRepository>,
}

impl GetSourceContentHandler {
    pub fn new(sources: Arc<dyn SourceRepository>, snapshots: Arc<dyn SnapshotRepository>) -> Self {
        Self { sources, snapshots }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: GetSourceContentCommand) -> Result<String, AppError> {
        let source_id = SourceId::parse(&command.source_id).map_err(|_| AppError::NotFound)?;
        self.sources
            .find_by_id(&command.user_id, &source_id)
            .await?
            .ok_or(AppError::NotFound)?;
        let snapshot = self
            .snapshots
            .find_by_source(&source_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if snapshot.body().is_empty() {
            return Err(AppError::NotFound);
        }
        Ok(String::from_utf8_lossy(snapshot.body()).into_owned())
    }
}
