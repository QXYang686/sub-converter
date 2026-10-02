use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use worker::d1::{D1DatabaseSession, D1PreparedStatement, D1Type};
use worker::send::SendFuture;

use crate::domain::{
    ExtractedConfig, RepositoryError, SnapshotMeta, SnapshotRepository, SourceId, SourceSnapshot,
    SubscriptionUserInfo,
};

const SNAPSHOT_COLUMNS: &str = "source_id, body, etag, last_modified, fetched_at, body_hash, \
     proxy_count, group_count, rule_count, protocol_counts, userinfo_upload, userinfo_download, \
     userinfo_total, userinfo_expire, update_interval, provider_name, provider_url, last_error";

const EXTRACTION_BATCH_SIZE: usize = 50;

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
    body_hash: Option<String>,
    proxy_count: Option<i64>,
    group_count: Option<i64>,
    rule_count: Option<i64>,
    protocol_counts: Option<String>,
    userinfo_upload: Option<i64>,
    userinfo_download: Option<i64>,
    userinfo_total: Option<i64>,
    userinfo_expire: Option<i64>,
    update_interval: Option<i64>,
    provider_name: Option<String>,
    provider_url: Option<String>,
    last_error: Option<String>,
}

fn parse_protocol_counts(raw: Option<&str>) -> BTreeMap<String, u32> {
    raw.and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default()
}

fn count(value: Option<i64>) -> u32 {
    value.unwrap_or(0).max(0) as u32
}

fn row_to_snapshot(row: SnapshotRow) -> Result<SourceSnapshot, RepositoryError> {
    let source_id = SourceId::parse(&row.source_id)
        .map_err(|err| RepositoryError::Unavailable(err.to_string()))?;
    let meta = SnapshotMeta {
        body_hash: row.body_hash,
        proxy_count: count(row.proxy_count),
        group_count: count(row.group_count),
        rule_count: count(row.rule_count),
        protocol_counts: parse_protocol_counts(row.protocol_counts.as_deref()),
        userinfo: SubscriptionUserInfo {
            upload: row.userinfo_upload,
            download: row.userinfo_download,
            total: row.userinfo_total,
            expire: row.userinfo_expire,
        },
        update_interval: row.update_interval,
        provider_name: row.provider_name,
        provider_url: row.provider_url,
        last_error: row.last_error,
    };
    Ok(SourceSnapshot::restore(
        source_id,
        row.body
            .map(serde_bytes::ByteBuf::into_vec)
            .unwrap_or_default(),
        row.etag,
        row.last_modified,
        row.fetched_at.unwrap_or(0),
        meta,
    ))
}

fn option_text(value: Option<&str>) -> D1Type<'_> {
    match value {
        Some(value) => D1Type::Text(value),
        None => D1Type::Null,
    }
}

fn option_number(value: Option<i64>) -> D1Type<'static> {
    match value {
        Some(value) => D1Type::Real(value as f64),
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
        let meta = snapshot.meta().clone();
        let protocol_counts = serde_json::to_string(&meta.protocol_counts).ok();
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO source_snapshots \
                     (source_id, body, etag, last_modified, fetched_at, body_hash, proxy_count, \
                      group_count, rule_count, protocol_counts, userinfo_upload, userinfo_download, \
                      userinfo_total, userinfo_expire, update_interval, provider_name, provider_url, \
                      refresh_lease_until, last_error) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, \
                      ?16, ?17, NULL, NULL) \
                     ON CONFLICT(source_id) DO UPDATE SET \
                       body = excluded.body, \
                       etag = excluded.etag, \
                       last_modified = excluded.last_modified, \
                       fetched_at = excluded.fetched_at, \
                       body_hash = excluded.body_hash, \
                       proxy_count = excluded.proxy_count, \
                       group_count = excluded.group_count, \
                       rule_count = excluded.rule_count, \
                       protocol_counts = excluded.protocol_counts, \
                       userinfo_upload = excluded.userinfo_upload, \
                       userinfo_download = excluded.userinfo_download, \
                       userinfo_total = excluded.userinfo_total, \
                       userinfo_expire = excluded.userinfo_expire, \
                       update_interval = excluded.update_interval, \
                       provider_name = excluded.provider_name, \
                       provider_url = excluded.provider_url, \
                       refresh_lease_until = NULL, \
                       last_error = NULL",
                )
                .bind_refs(&[
                    D1Type::Text(&source_id),
                    D1Type::Blob(&body),
                    option_text(etag.as_deref()),
                    option_text(last_modified.as_deref()),
                    D1Type::Real(fetched_at),
                    option_text(meta.body_hash.as_deref()),
                    D1Type::Integer(meta.proxy_count as i32),
                    D1Type::Integer(meta.group_count as i32),
                    D1Type::Integer(meta.rule_count as i32),
                    option_text(protocol_counts.as_deref()),
                    option_number(meta.userinfo.upload),
                    option_number(meta.userinfo.download),
                    option_number(meta.userinfo.total),
                    option_number(meta.userinfo.expire),
                    option_number(meta.update_interval),
                    option_text(meta.provider_name.as_deref()),
                    option_text(meta.provider_url.as_deref()),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn replace_extraction(
        &self,
        source_id: &SourceId,
        extraction: &ExtractedConfig,
    ) -> Result<(), RepositoryError> {
        let source_id = source_id.to_string();
        let extraction = extraction.clone();
        SendFuture::new(async move {
            let mut statements: Vec<D1PreparedStatement> = Vec::new();
            for table in ["source_proxies", "source_proxy_groups", "source_config"] {
                statements.push(
                    self.db
                        .prepare(format!("DELETE FROM {table} WHERE source_id = ?1"))
                        .bind_refs(&[D1Type::Text(&source_id)])?,
                );
            }

            for (ordinal, proxy) in extraction.proxies.iter().enumerate() {
                statements.push(
                    self.db
                        .prepare(
                            "INSERT INTO source_proxies \
                             (source_id, ordinal, protocol, name, server, port, options) \
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        )
                        .bind_refs(&[
                            D1Type::Text(&source_id),
                            D1Type::Integer(ordinal as i32),
                            option_text(proxy.protocol.as_deref()),
                            option_text(proxy.name.as_deref()),
                            option_text(proxy.server.as_deref()),
                            option_number(proxy.port.map(i64::from)),
                            D1Type::Text(&proxy.options_json),
                        ])?,
                );
            }

            for (ordinal, group) in extraction.groups.iter().enumerate() {
                statements.push(
                    self.db
                        .prepare(
                            "INSERT INTO source_proxy_groups \
                             (source_id, ordinal, name, group_type, proxies, options) \
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        )
                        .bind_refs(&[
                            D1Type::Text(&source_id),
                            D1Type::Integer(ordinal as i32),
                            option_text(group.name.as_deref()),
                            option_text(group.group_type.as_deref()),
                            D1Type::Text(&group.proxies_json),
                            D1Type::Text(&group.options_json),
                        ])?,
                );
            }

            let rules_json = extraction.rules_json();
            let settings_json = extraction.settings_json();
            statements.push(
                self.db
                    .prepare(
                        "INSERT INTO source_config (source_id, rules, settings) \
                         VALUES (?1, ?2, ?3)",
                    )
                    .bind_refs(&[
                        D1Type::Text(&source_id),
                        D1Type::Text(&rules_json),
                        D1Type::Text(&settings_json),
                    ])?,
            );

            for chunk in statements.chunks(EXTRACTION_BATCH_SIZE) {
                self.db.batch(chunk.to_vec()).await?;
            }
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
                .batch(vec![
                    self.db
                        .prepare("DELETE FROM source_proxies WHERE source_id = ?1")
                        .bind_refs(&[D1Type::Text(&source_id)])?,
                    self.db
                        .prepare("DELETE FROM source_proxy_groups WHERE source_id = ?1")
                        .bind_refs(&[D1Type::Text(&source_id)])?,
                    self.db
                        .prepare("DELETE FROM source_config WHERE source_id = ?1")
                        .bind_refs(&[D1Type::Text(&source_id)])?,
                    self.db
                        .prepare("DELETE FROM source_snapshots WHERE source_id = ?1")
                        .bind_refs(&[D1Type::Text(&source_id)])?,
                ])
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
