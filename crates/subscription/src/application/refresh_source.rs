use std::sync::Arc;

use crate::domain::{
    body_hash, count_provider_rules, document as parse_document,
    parse_content_disposition_filename, parse_subscription_userinfo, parse_value, ExtractedConfig,
    ExtractedRuleProvider, PublicationRepository, PublicationSnapshotRepository,
    RuleProviderRepository, RuleProviderSnapshot, SnapshotMeta, SnapshotRepository, SourceId,
    SourceRepository, SourceSnapshot, SourceUrl,
};

use super::error::AppError;
use super::ports::{BackgroundTasks, Clock, FetchOutcome, FetchValidators, Fetcher};
use super::rebuild_publication_snapshot::RebuildPublicationSnapshotHandler;

pub const REFRESH_LEASE_SECONDS: i64 = 120;

/// 单次源刷新最多代抓的 rule-provider 数量，避免订阅声明过多拖垮后台任务。
pub const MAX_RULE_PROVIDERS_PER_REFRESH: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshStatus {
    Refreshed,
    NotModified,
    NoData,
    Failed,
    InProgress,
}

pub struct RefreshSourceHandler {
    fetcher: Arc<dyn Fetcher>,
    snapshots: Arc<dyn SnapshotRepository>,
    rule_providers: Arc<dyn RuleProviderRepository>,
    sources: Arc<dyn SourceRepository>,
    publications: Arc<dyn PublicationRepository>,
    publication_snapshots: Arc<dyn PublicationSnapshotRepository>,
    clock: Arc<dyn Clock>,
}

impl RefreshSourceHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fetcher: Arc<dyn Fetcher>,
        snapshots: Arc<dyn SnapshotRepository>,
        sources: Arc<dyn SourceRepository>,
        publications: Arc<dyn PublicationRepository>,
        publication_snapshots: Arc<dyn PublicationSnapshotRepository>,
        clock: Arc<dyn Clock>,
        rule_providers: Arc<dyn RuleProviderRepository>,
    ) -> Self {
        Self {
            fetcher,
            snapshots,
            rule_providers,
            sources,
            publications,
            publication_snapshots,
            clock,
        }
    }

    pub fn spawn(&self, background: &dyn BackgroundTasks, source_id: SourceId, url: SourceUrl) {
        let fetcher = self.fetcher.clone();
        let snapshots = self.snapshots.clone();
        let rule_providers = self.rule_providers.clone();
        let sources = self.sources.clone();
        let publications = self.publications.clone();
        let publication_snapshots = self.publication_snapshots.clone();
        let clock = self.clock.clone();
        background.spawn(Box::pin(async move {
            let handler = RefreshSourceHandler {
                fetcher,
                snapshots,
                rule_providers,
                sources,
                publications,
                publication_snapshots,
                clock,
            };
            if let Err(err) = handler.handle(&source_id, &url).await {
                tracing::warn!(source_id = %source_id, error = %err, "source refresh failed");
            }
        }));
    }

    async fn rebuild_publications(&self, source_id: &SourceId) {
        let publications = match self.publications.list_by_source_id(source_id).await {
            Ok(publications) => publications,
            Err(err) => {
                tracing::warn!(source_id = %source_id, error = %err, "failed to list publications for rebuild");
                return;
            }
        };
        for publication in &publications {
            let rebuild = RebuildPublicationSnapshotHandler::new(
                self.sources.clone(),
                self.snapshots.clone(),
                self.publication_snapshots.clone(),
                self.clock.clone(),
            );
            if let Err(err) = rebuild.handle(publication).await {
                tracing::warn!(
                    publication_id = %publication.id(),
                    error = %err,
                    "publication snapshot rebuild failed"
                );
            }
        }
    }

    #[tracing::instrument(skip_all, fields(source_id = %source_id))]
    pub async fn handle(
        &self,
        source_id: &SourceId,
        url: &SourceUrl,
    ) -> Result<RefreshStatus, AppError> {
        let now = self.clock.now();
        let previous = self.snapshots.find_by_source(source_id).await?;
        let validators = match &previous {
            Some(snapshot) => FetchValidators {
                etag: snapshot.etag().map(str::to_string),
                last_modified: snapshot.last_modified().map(str::to_string),
            },
            None => FetchValidators::default(),
        };

        if !self
            .snapshots
            .try_acquire_lease(source_id, now, now + REFRESH_LEASE_SECONDS)
            .await?
        {
            return Ok(RefreshStatus::InProgress);
        }

        match self.fetcher.fetch(url, &validators).await {
            Ok(FetchOutcome::Fetched(document)) => {
                let Some(text) = std::str::from_utf8(&document.body).ok() else {
                    self.snapshots
                        .record_error(source_id, "response is not valid utf-8")
                        .await?;
                    return Ok(RefreshStatus::NoData);
                };
                let Ok(document_value) = parse_document(text) else {
                    self.snapshots
                        .record_error(source_id, "response is not valid yaml")
                        .await?;
                    return Ok(RefreshStatus::NoData);
                };
                if parse_value(&document_value).is_empty() {
                    self.snapshots
                        .record_error(source_id, "no usable clash proxies")
                        .await?;
                    tracing::warn!(source_id = %source_id, "source refresh found no usable proxies");
                    return Ok(RefreshStatus::NoData);
                }

                let extraction = ExtractedConfig::from_value(&document_value);
                let hash = body_hash(&document.body);
                let unchanged = previous
                    .as_ref()
                    .and_then(|snapshot| snapshot.meta().body_hash.as_deref())
                    == Some(hash.as_str());

                let protocol_counts = extraction
                    .protocol_counts()
                    .into_iter()
                    .map(|(protocol, count)| (protocol, count as u32))
                    .collect();
                let meta = SnapshotMeta {
                    body_hash: Some(hash),
                    proxy_count: extraction.proxy_count() as u32,
                    group_count: extraction.group_count() as u32,
                    rule_count: extraction.rule_count() as u32,
                    protocol_counts,
                    userinfo: document
                        .subscription_userinfo
                        .as_deref()
                        .map(parse_subscription_userinfo)
                        .unwrap_or_default(),
                    update_interval: document
                        .profile_update_interval
                        .as_deref()
                        .and_then(|value| value.trim().parse::<i64>().ok()),
                    provider_name: document
                        .content_disposition
                        .as_deref()
                        .and_then(parse_content_disposition_filename),
                    provider_url: document.profile_web_page_url,
                    last_error: None,
                };
                let snapshot = SourceSnapshot::restore(
                    source_id.clone(),
                    document.body,
                    document.etag,
                    document.last_modified,
                    now,
                    meta,
                );
                self.snapshots.save(&snapshot).await?;
                let needs_extraction = if unchanged {
                    !self.snapshots.has_extraction(source_id).await?
                } else {
                    true
                };
                if needs_extraction {
                    self.snapshots
                        .replace_extraction(source_id, &extraction)
                        .await?;
                    self.rule_providers
                        .replace_providers(source_id, &extraction.rule_providers)
                        .await?;
                }
                self.refresh_provider_snapshots(source_id, &extraction.rule_providers)
                    .await;
                self.rebuild_publications(source_id).await;
                Ok(RefreshStatus::Refreshed)
            }
            Ok(FetchOutcome::NotModified) => {
                self.snapshots.touch(source_id, now).await?;
                if let Some(previous) = previous.as_ref() {
                    let needs_backfill = previous.meta().body_hash.is_none()
                        || !self.snapshots.has_extraction(source_id).await?;
                    if needs_backfill {
                        self.backfill(source_id, previous, now).await?;
                        self.rebuild_publications(source_id).await;
                    }
                }
                if let Ok(providers) = self.rule_providers.list_providers(source_id).await {
                    self.refresh_provider_snapshots(source_id, &providers).await;
                }
                Ok(RefreshStatus::NotModified)
            }
            Err(err) => {
                self.snapshots
                    .record_error(source_id, &err.to_string())
                    .await?;
                tracing::warn!(source_id = %source_id, error = %err, "source refresh failed");
                Ok(RefreshStatus::Failed)
            }
        }
    }

    async fn backfill(
        &self,
        source_id: &SourceId,
        previous: &SourceSnapshot,
        now: i64,
    ) -> Result<(), AppError> {
        if previous.body().is_empty() {
            return Ok(());
        }
        let Ok(text) = std::str::from_utf8(previous.body()) else {
            return Ok(());
        };
        let Ok(document) = parse_document(text) else {
            return Ok(());
        };
        if parse_value(&document).is_empty() {
            return Ok(());
        }

        let extraction = ExtractedConfig::from_value(&document);
        let mut meta = previous.meta().clone();
        meta.body_hash = Some(body_hash(previous.body()));
        meta.proxy_count = extraction.proxy_count() as u32;
        meta.group_count = extraction.group_count() as u32;
        meta.rule_count = extraction.rule_count() as u32;
        meta.protocol_counts = extraction
            .protocol_counts()
            .into_iter()
            .map(|(protocol, count)| (protocol, count as u32))
            .collect();
        meta.last_error = None;

        let snapshot = SourceSnapshot::restore(
            source_id.clone(),
            previous.body().to_vec(),
            previous.etag().map(str::to_string),
            previous.last_modified().map(str::to_string),
            now,
            meta,
        );
        self.snapshots.save(&snapshot).await?;
        self.snapshots
            .replace_extraction(source_id, &extraction)
            .await?;
        self.rule_providers
            .replace_providers(source_id, &extraction.rule_providers)
            .await?;
        Ok(())
    }

    async fn refresh_provider_snapshots(
        &self,
        source_id: &SourceId,
        providers: &[ExtractedRuleProvider],
    ) {
        let now = self.clock.now();
        for provider in providers
            .iter()
            .filter(|provider| provider.is_remote())
            .take(MAX_RULE_PROVIDERS_PER_REFRESH)
        {
            let Some(url) = provider
                .url
                .as_deref()
                .and_then(|raw| SourceUrl::new(raw).ok())
            else {
                continue;
            };
            let previous = match self
                .rule_providers
                .find_snapshot(source_id, &provider.name)
                .await
            {
                Ok(snapshot) => snapshot,
                Err(err) => {
                    tracing::warn!(
                        source_id = %source_id,
                        provider = %provider.name,
                        error = %err,
                        "failed to read rule provider snapshot"
                    );
                    continue;
                }
            };
            let validators = previous
                .as_ref()
                .map(|snapshot| FetchValidators {
                    etag: snapshot.etag().map(str::to_string),
                    last_modified: snapshot.last_modified().map(str::to_string),
                })
                .unwrap_or_default();

            match self.fetcher.fetch(&url, &validators).await {
                Ok(FetchOutcome::Fetched(document)) => {
                    let hash = body_hash(&document.body);
                    let rule_count = count_provider_rules(&document.body);
                    let snapshot = RuleProviderSnapshot::restore(
                        source_id.clone(),
                        provider.name.clone(),
                        document.body,
                        document.etag,
                        document.last_modified,
                        now,
                        hash,
                        rule_count,
                        None,
                    );
                    if let Err(err) = self.rule_providers.save_snapshot(&snapshot).await {
                        tracing::warn!(
                            source_id = %source_id,
                            provider = %provider.name,
                            error = %err,
                            "failed to save rule provider snapshot"
                        );
                    }
                }
                Ok(FetchOutcome::NotModified) => {
                    if previous.is_some() {
                        if let Err(err) = self
                            .rule_providers
                            .touch_snapshot(source_id, &provider.name, now)
                            .await
                        {
                            tracing::warn!(
                                source_id = %source_id,
                                provider = %provider.name,
                                error = %err,
                                "failed to touch rule provider snapshot"
                            );
                        }
                    }
                }
                Err(err) => {
                    if let Err(save_err) = self
                        .rule_providers
                        .record_snapshot_error(source_id, &provider.name, &err.to_string())
                        .await
                    {
                        tracing::warn!(
                            source_id = %source_id,
                            provider = %provider.name,
                            error = %save_err,
                            "failed to record rule provider error"
                        );
                    }
                }
            }
        }
    }
}
