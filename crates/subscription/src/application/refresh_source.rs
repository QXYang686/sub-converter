use std::sync::Arc;

use crate::domain::{
    parse as parse_clash, SnapshotRepository, SourceId, SourceSnapshot, SourceUrl,
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
        let validators = match self.snapshots.find_by_source(source_id).await? {
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
                let usable = std::str::from_utf8(&document.body)
                    .ok()
                    .and_then(|body| parse_clash(body).ok())
                    .is_some_and(|parsed| !parsed.is_empty());
                if !usable {
                    self.snapshots
                        .record_error(source_id, "no supported clash proxies")
                        .await?;
                    tracing::warn!(source_id = %source_id, "source refresh found no usable proxies");
                    return Ok(RefreshStatus::NoData);
                }

                let snapshot = SourceSnapshot::restore(
                    source_id.clone(),
                    document.body,
                    document.etag,
                    document.last_modified,
                    now,
                );
                self.snapshots.save(&snapshot).await?;
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
