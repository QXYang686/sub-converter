use std::sync::Arc;

use crate::domain::{
    body_hash, document as parse_document, parse_content_disposition_filename,
    parse_subscription_userinfo, parse_value, ExtractedConfig, SnapshotMeta, SnapshotRepository,
    SourceId, SourceSnapshot, SourceUrl,
};

use super::error::AppError;
use super::ports::{BackgroundTasks, Clock, FetchOutcome, FetchValidators, Fetcher};

pub const REFRESH_LEASE_SECONDS: i64 = 120;

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
    clock: Arc<dyn Clock>,
}

impl RefreshSourceHandler {
    pub fn new(
        fetcher: Arc<dyn Fetcher>,
        snapshots: Arc<dyn SnapshotRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            fetcher,
            snapshots,
            clock,
        }
    }

    pub fn spawn(&self, background: &dyn BackgroundTasks, source_id: SourceId, url: SourceUrl) {
        let fetcher = self.fetcher.clone();
        let snapshots = self.snapshots.clone();
        let clock = self.clock.clone();
        background.spawn(Box::pin(async move {
            let handler = RefreshSourceHandler {
                fetcher,
                snapshots,
                clock,
            };
            if let Err(err) = handler.handle(&source_id, &url).await {
                tracing::warn!(source_id = %source_id, error = %err, "source refresh failed");
            }
        }));
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
                if !unchanged {
                    self.snapshots
                        .replace_extraction(source_id, &extraction)
                        .await?;
                }
                Ok(RefreshStatus::Refreshed)
            }
            Ok(FetchOutcome::NotModified) => {
                self.snapshots.touch(source_id, now).await?;
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
}
