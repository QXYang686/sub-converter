use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use worker::d1::{D1DatabaseSession, D1Type};
use worker::send::SendFuture;

use crate::domain::{RepositoryError, SnapshotRepository, SourceId, SourceSnapshot};

const SNAPSHOT_COLUMNS: &str = "source_id, body, etag, last_modified, fetched_at";

pub struct D1SnapshotRepository {
    db: Arc<D1DatabaseSession>,
}

impl D1SnapshotRepository {
    pub fn new(db: Arc<D1DatabaseSession>) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct SnapshotRow {
    source_id: String,
    #[serde(default)]
    body: Option<serde_bytes::ByteBuf>,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: Option<i64>,
}

fn row_to_snapshot(row: SnapshotRow) -> Result<SourceSnapshot, RepositoryError> {
    let source_id = SourceId::parse(&row.source_id)
        .map_err(|err| RepositoryError::Unavailable(err.to_string()))?;
    Ok(SourceSnapshot::restore(
        source_id,
        row.body
            .map(serde_bytes::ByteBuf::into_vec)
            .unwrap_or_default(),
        row.etag,
        row.last_modified,
        row.fetched_at.unwrap_or(0),
    ))
}

fn option_text(value: Option<&str>) -> D1Type<'_> {
    match value {
        Some(value) => D1Type::Text(value),
        None => D1Type::Null,
    }
}

#[async_trait]
impl SnapshotRepository for D1SnapshotRepository {
    async fn find_by_source(
        &self,
        source_id: &SourceId,
    ) -> Result<Option<SourceSnapshot>, RepositoryError> {
        let source_id = source_id.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {SNAPSHOT_COLUMNS} FROM source_snapshots WHERE source_id = ?1"
                ))
                .bind_refs(&[D1Type::Text(&source_id)])?
                .first::<SnapshotRow>(None)
                .await?;
            row.map(row_to_snapshot).transpose()
        })
        .await
    }

    async fn list_by_sources(
        &self,
        source_ids: &[SourceId],
    ) -> Result<Vec<SourceSnapshot>, RepositoryError> {
        if source_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<String> = source_ids.iter().map(ToString::to_string).collect();
        let placeholders = (1..=ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        SendFuture::new(async move {
            let bindings: Vec<D1Type> = ids.iter().map(|id| D1Type::Text(id)).collect();
            let result = self
                .db
                .prepare(format!(
                    "SELECT {SNAPSHOT_COLUMNS} FROM source_snapshots \
                     WHERE body IS NOT NULL AND source_id IN ({placeholders})"
                ))
                .bind_refs(&bindings)?
                .all()
                .await?;
            result
                .results::<SnapshotRow>()?
                .into_iter()
                .map(row_to_snapshot)
                .collect()
        })
        .await
    }

    async fn save(&self, snapshot: &SourceSnapshot) -> Result<(), RepositoryError> {
        let source_id = snapshot.source_id().to_string();
        let body = snapshot.body().to_vec();
        let etag = snapshot.etag().map(str::to_string);
        let last_modified = snapshot.last_modified().map(str::to_string);
        let fetched_at = snapshot.fetched_at() as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO source_snapshots \
                     (source_id, body, etag, last_modified, fetched_at, refresh_lease_until, last_error) \
                     VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL) \
                     ON CONFLICT(source_id) DO UPDATE SET \
                       body = excluded.body, \
                       etag = excluded.etag, \
                       last_modified = excluded.last_modified, \
                       fetched_at = excluded.fetched_at, \
                       refresh_lease_until = NULL, \
                       last_error = NULL",
                )
                .bind_refs(&[
                    D1Type::Text(&source_id),
                    D1Type::Blob(&body),
                    option_text(etag.as_deref()),
                    option_text(last_modified.as_deref()),
                    D1Type::Real(fetched_at),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn touch(&self, source_id: &SourceId, fetched_at: i64) -> Result<(), RepositoryError> {
        let source_id = source_id.to_string();
        let fetched_at = fetched_at as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "UPDATE source_snapshots \
                     SET fetched_at = ?1, refresh_lease_until = NULL, last_error = NULL \
                     WHERE source_id = ?2",
                )
                .bind_refs(&[D1Type::Real(fetched_at), D1Type::Text(&source_id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn record_error(&self, source_id: &SourceId, error: &str) -> Result<(), RepositoryError> {
        let source_id = source_id.to_string();
        let error = error.to_string();
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO source_snapshots (source_id, last_error, refresh_lease_until) \
                     VALUES (?1, ?2, NULL) \
                     ON CONFLICT(source_id) DO UPDATE SET \
                       last_error = excluded.last_error, \
                       refresh_lease_until = NULL",
                )
                .bind_refs(&[D1Type::Text(&source_id), D1Type::Text(&error)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn clear(&self, source_id: &SourceId) -> Result<(), RepositoryError> {
        let source_id = source_id.to_string();
        SendFuture::new(async move {
            self.db
                .prepare("DELETE FROM source_snapshots WHERE source_id = ?1")
                .bind_refs(&[D1Type::Text(&source_id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn try_acquire_lease(
        &self,
        source_id: &SourceId,
        now: i64,
        lease_until: i64,
    ) -> Result<bool, RepositoryError> {
        let source_id = source_id.to_string();
        let now = now as f64;
        let lease_until = lease_until as f64;
        SendFuture::new(async move {
            let result = self
                .db
                .prepare(
                    "INSERT INTO source_snapshots (source_id, refresh_lease_until) \
                     VALUES (?1, ?3) \
                     ON CONFLICT(source_id) DO UPDATE SET \
                       refresh_lease_until = excluded.refresh_lease_until \
                     WHERE source_snapshots.refresh_lease_until IS NULL \
                        OR source_snapshots.refresh_lease_until < ?2",
                )
                .bind_refs(&[
                    D1Type::Text(&source_id),
                    D1Type::Real(now),
                    D1Type::Real(lease_until),
                ])?
                .run()
                .await?;
            Ok(result.meta()?.and_then(|meta| meta.changes).unwrap_or(0) == 1)
        })
        .await
    }
}
