use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::{
    merge, parse as parse_clash, render, PublicationRepository, PublicationSecret,
    SnapshotRepository, Source, SourceId, SourceRepository, SourceSnapshot,
};

use super::error::AppError;
use super::ports::{BackgroundTasks, Clock, SubscriptionFormat};
use super::refresh_source::RefreshSourceHandler;

#[derive(Debug, Clone)]
pub struct ServePublicationCommand {
    pub secret: String,
    pub format: SubscriptionFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSubscription {
    pub name: String,
    pub content: String,
}

pub struct ServePublicationHandler {
    publications: Arc<dyn PublicationRepository>,
    sources: Arc<dyn SourceRepository>,
    snapshots: Arc<dyn SnapshotRepository>,
    refresher: Arc<RefreshSourceHandler>,
    background: Arc<dyn BackgroundTasks>,
    clock: Arc<dyn Clock>,
}

impl ServePublicationHandler {
    pub fn new(
        publications: Arc<dyn PublicationRepository>,
        sources: Arc<dyn SourceRepository>,
        snapshots: Arc<dyn SnapshotRepository>,
        refresher: Arc<RefreshSourceHandler>,
        background: Arc<dyn BackgroundTasks>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            publications,
            sources,
            snapshots,
            refresher,
            background,
            clock,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: ServePublicationCommand,
    ) -> Result<GeneratedSubscription, AppError> {
        let secret = PublicationSecret::new(&command.secret).map_err(|_| AppError::NotFound)?;
        let publication = self
            .publications
            .find_by_secret(&secret)
            .await?
            .ok_or(AppError::NotFound)?;

        let now = self.clock.now();
        let expired = publication
            .expires_at()
            .is_some_and(|expires_at| expires_at <= now);
        if !publication.enabled() || expired {
            return Err(AppError::NotFound);
        }

        let all_sources = self.sources.list_by_user(publication.user_id()).await?;
        let by_id: HashMap<&SourceId, &Source> = all_sources
            .iter()
            .map(|source| (source.id(), source))
            .collect();
        let ordered: Vec<&Source> = publication
            .sources()
            .iter()
            .filter_map(|binding| by_id.get(binding.source_id()).copied())
            .filter(|source| source.enabled())
            .collect();

        for source in &ordered {
            self.refresher.spawn(
                self.background.as_ref(),
                source.id().clone(),
                source.url().clone(),
            );
        }

        let source_ids: Vec<SourceId> = ordered.iter().map(|source| source.id().clone()).collect();
        let snapshots = self.snapshots.list_by_sources(&source_ids).await?;
        let bodies: HashMap<&SourceId, &SourceSnapshot> = snapshots
            .iter()
            .map(|snapshot| (snapshot.source_id(), snapshot))
            .collect();

        let mut parsed = Vec::new();
        for source in &ordered {
            let Some(snapshot) = bodies.get(source.id()) else {
                continue;
            };
            let Ok(text) = std::str::from_utf8(snapshot.body()) else {
                continue;
            };
            match parse_clash(text) {
                Ok(result) if !result.is_empty() => parsed.push(result),
                Ok(result) => tracing::debug!(
                    source_id = %source.id(),
                    skipped = result.skipped(),
                    "source snapshot has no supported proxies"
                ),
                Err(err) => tracing::debug!(
                    source_id = %source.id(),
                    error = %err,
                    "source snapshot is not a clash config"
                ),
            }
        }

        let merged = merge(parsed.iter());
        let content = match command.format {
            SubscriptionFormat::Clash => render(&merged),
        };
        Ok(GeneratedSubscription {
            name: publication.name().value().to_string(),
            content,
        })
    }
}
