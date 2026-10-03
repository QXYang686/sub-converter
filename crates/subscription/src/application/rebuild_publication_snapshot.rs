use std::sync::Arc;

use crate::domain::{
    Publication, PublicationSnapshot, PublicationSnapshotRepository, SnapshotRepository,
    SourceRepository,
};

use super::error::AppError;
use super::ports::{Clock, SubscriptionFormat};
use super::serve_publication::{build_subscription, ordered_enabled_sources};

pub struct RebuildPublicationSnapshotHandler {
    sources: Arc<dyn SourceRepository>,
    snapshots: Arc<dyn SnapshotRepository>,
    publication_snapshots: Arc<dyn PublicationSnapshotRepository>,
    clock: Arc<dyn Clock>,
}

impl RebuildPublicationSnapshotHandler {
    pub fn new(
        sources: Arc<dyn SourceRepository>,
        snapshots: Arc<dyn SnapshotRepository>,
        publication_snapshots: Arc<dyn PublicationSnapshotRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            sources,
            snapshots,
            publication_snapshots,
            clock,
        }
    }

    #[tracing::instrument(skip_all, fields(publication_id = %publication.id()))]
    pub async fn handle(&self, publication: &Publication) -> Result<(), AppError> {
        let ordered = ordered_enabled_sources(self.sources.as_ref(), publication).await?;
        let source_ids: Vec<_> = ordered.iter().map(|source| source.id().clone()).collect();
        let snapshots = self.snapshots.list_by_sources(&source_ids).await?;

        let format = SubscriptionFormat::Clash;
        let generated = build_subscription(publication, &ordered, &snapshots, format);
        let snapshot = PublicationSnapshot::restore(
            publication.id().clone(),
            format.as_str().to_string(),
            generated.content,
            generated.userinfo,
            self.clock.now(),
        );
        self.publication_snapshots.save(&snapshot).await?;
        Ok(())
    }
}
