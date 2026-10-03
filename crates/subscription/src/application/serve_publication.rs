use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::{
    merge_configs, render_config, Publication, PublicationRepository, PublicationSecret,
    PublicationSnapshot, PublicationSnapshotRepository, SnapshotRepository, Source,
    SourceExtraction, SourceId, SourceRepository, SubscriptionUserInfo,
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
    pub userinfo: SubscriptionUserInfo,
}

pub struct ServePublicationHandler {
    publications: Arc<dyn PublicationRepository>,
    sources: Arc<dyn SourceRepository>,
    snapshots: Arc<dyn SnapshotRepository>,
    publication_snapshots: Arc<dyn PublicationSnapshotRepository>,
    refresher: Arc<RefreshSourceHandler>,
    background: Arc<dyn BackgroundTasks>,
    clock: Arc<dyn Clock>,
}

impl ServePublicationHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        publications: Arc<dyn PublicationRepository>,
        sources: Arc<dyn SourceRepository>,
        snapshots: Arc<dyn SnapshotRepository>,
        publication_snapshots: Arc<dyn PublicationSnapshotRepository>,
        refresher: Arc<RefreshSourceHandler>,
        background: Arc<dyn BackgroundTasks>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            publications,
            sources,
            snapshots,
            publication_snapshots,
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

        let ordered = ordered_enabled_sources(self.sources.as_ref(), &publication).await?;
        for source in &ordered {
            self.refresher.spawn(
                self.background.as_ref(),
                source.id().clone(),
                source.url().clone(),
            );
        }

        let format = command.format.as_str();
        if let Some(cached) = self
            .publication_snapshots
            .find(publication.id(), format)
            .await?
        {
            return Ok(GeneratedSubscription {
                name: publication.name().value().to_string(),
                content: cached.content().to_string(),
                userinfo: cached.userinfo().clone(),
            });
        }

        let source_ids: Vec<SourceId> = ordered.iter().map(|source| source.id().clone()).collect();
        let extractions = self.snapshots.list_extractions(&source_ids).await?;
        let generated = build_subscription(&publication, &ordered, &extractions, command.format);
        let snapshot = PublicationSnapshot::restore(
            publication.id().clone(),
            format.to_string(),
            generated.content.clone(),
            generated.userinfo.clone(),
            now,
        );
        self.publication_snapshots.save(&snapshot).await?;
        Ok(generated)
    }
}

pub(super) async fn ordered_enabled_sources(
    sources: &dyn SourceRepository,
    publication: &Publication,
) -> Result<Vec<Source>, AppError> {
    let all_sources = sources.list_by_user(publication.user_id()).await?;
    let by_id: HashMap<&SourceId, &Source> = all_sources
        .iter()
        .map(|source| (source.id(), source))
        .collect();
    Ok(publication
        .sources()
        .iter()
        .filter_map(|binding| {
            by_id
                .get(binding.source_id())
                .map(|source| (*source).clone())
        })
        .filter(|source| source.enabled())
        .collect())
}

pub(super) fn build_subscription(
    publication: &Publication,
    ordered: &[Source],
    extractions: &[SourceExtraction],
    format: SubscriptionFormat,
) -> GeneratedSubscription {
    let by_source: HashMap<&SourceId, &SourceExtraction> = extractions
        .iter()
        .map(|extraction| (&extraction.source_id, extraction))
        .collect();

    let mut configs = Vec::new();
    let mut userinfo = SubscriptionUserInfo::default();
    for source in ordered {
        let Some(extraction) = by_source.get(source.id()) else {
            continue;
        };
        aggregate_userinfo(&mut userinfo, &extraction.userinfo);
        if extraction.config.proxies.is_empty() {
            tracing::debug!(
                source_id = %source.id(),
                "source snapshot has no extracted proxies"
            );
            continue;
        }
        configs.push(&extraction.config);
    }

    let merged = merge_configs(configs);
    let content = match format {
        SubscriptionFormat::Clash => render_config(&merged),
    };
    GeneratedSubscription {
        name: publication.name().value().to_string(),
        content,
        userinfo,
    }
}

fn aggregate_userinfo(total: &mut SubscriptionUserInfo, source: &SubscriptionUserInfo) {
    total.upload = add_optional(total.upload, source.upload);
    total.download = add_optional(total.download, source.download);
    total.total = add_optional(total.total, source.total);
    total.expire = match (total.expire, source.expire) {
        (Some(current), Some(incoming)) => Some(current.min(incoming)),
        (current, incoming) => current.or(incoming),
    };
}

fn add_optional(current: Option<i64>, incoming: Option<i64>) -> Option<i64> {
    match (current, incoming) {
        (Some(current), Some(incoming)) => Some(current.saturating_add(incoming)),
        (current, incoming) => current.or(incoming),
    }
}
