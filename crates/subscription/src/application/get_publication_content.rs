use std::sync::Arc;

use user::UserId;

use crate::domain::{PublicationId, PublicationRepository, SnapshotRepository, SourceRepository};

use super::error::AppError;
use super::ports::SubscriptionFormat;
use super::serve_publication::{build_subscription, ordered_enabled_sources};

#[derive(Debug, Clone)]
pub struct GetPublicationContentCommand {
    pub user_id: UserId,
    pub publication_id: String,
}

/// 返回发布订阅按当前绑定合并后的 Clash 渲染结果，与公开分发内容一致。
pub struct GetPublicationContentHandler {
    publications: Arc<dyn PublicationRepository>,
    sources: Arc<dyn SourceRepository>,
    snapshots: Arc<dyn SnapshotRepository>,
}

impl GetPublicationContentHandler {
    pub fn new(
        publications: Arc<dyn PublicationRepository>,
        sources: Arc<dyn SourceRepository>,
        snapshots: Arc<dyn SnapshotRepository>,
    ) -> Self {
        Self {
            publications,
            sources,
            snapshots,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: GetPublicationContentCommand) -> Result<String, AppError> {
        let publication_id =
            PublicationId::parse(&command.publication_id).map_err(|_| AppError::NotFound)?;
        let publication = self
            .publications
            .find_by_id(&command.user_id, &publication_id)
            .await?
            .ok_or(AppError::NotFound)?;

        let ordered = ordered_enabled_sources(self.sources.as_ref(), &publication).await?;
        let source_ids: Vec<_> = ordered.iter().map(|source| source.id().clone()).collect();
        let extractions = self.snapshots.list_extractions(&source_ids).await?;
        let generated = build_subscription(
            &publication,
            &ordered,
            &extractions,
            SubscriptionFormat::Clash,
        );
        Ok(generated.content)
    }
}
