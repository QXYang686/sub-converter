use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use worker::d1::{D1DatabaseSession, D1PreparedStatement, D1Type};
use worker::send::SendFuture;

use crate::domain::{
    ExtractedRuleProvider, RepositoryError, RuleProviderRepository, RuleProviderSnapshot, SourceId,
};

const BATCH_SIZE: usize = 50;

const PROVIDER_COLUMNS: &str =
    "source_id, ordinal, name, provider_type, behavior, url, path, interval, options";

const SNAPSHOT_COLUMNS: &str =
    "source_id, name, body, etag, last_modified, fetched_at, body_hash, rule_count, last_error";

pub struct D1RuleProviderRepository {
    db: Arc<D1DatabaseSession>,
}

impl D1RuleProviderRepository {
    pub fn new(db: Arc<D1DatabaseSession>) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct ProviderRow {
    name: String,
    provider_type: Option<String>,
    behavior: Option<String>,
    url: Option<String>,
    path: Option<String>,
    interval: Option<i64>,
    options: String,
}

#[derive(Deserialize)]
struct SnapshotRow {
    source_id: String,
    name: String,
    #[serde(default)]
    body: Option<serde_bytes::ByteBuf>,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: Option<i64>,
    body_hash: Option<String>,
    rule_count: Option<i64>,
    last_error: Option<String>,
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

fn row_to_snapshot(row: SnapshotRow) -> Result<RuleProviderSnapshot, RepositoryError> {
    let source_id = SourceId::parse(&row.source_id)
        .map_err(|err| RepositoryError::Unavailable(err.to_string()))?;
    Ok(RuleProviderSnapshot::restore(
        source_id,
        row.name,
        row.body
            .map(serde_bytes::ByteBuf::into_vec)
            .unwrap_or_default(),
        row.etag,
        row.last_modified,
        row.fetched_at.unwrap_or(0),
        row.body_hash.unwrap_or_default(),
        row.rule_count.unwrap_or(0).max(0) as u32,
        row.last_error,
    ))
}

#[async_trait]
impl RuleProviderRepository for D1RuleProviderRepository {
    async fn replace_providers(
        &self,
        source_id: &SourceId,
        providers: &[ExtractedRuleProvider],
    ) -> Result<(), RepositoryError> {
        let source_id = source_id.to_string();
        let providers = providers.to_vec();
        SendFuture::new(async move {
            let mut statements: Vec<D1PreparedStatement> = vec![
                self.db
                    .prepare("DELETE FROM source_rule_providers WHERE source_id = ?1")
                    .bind_refs(&[D1Type::Text(&source_id)])?,
            ];

            let names: Vec<&str> = providers.iter().map(|provider| provider.name.as_str()).collect();
            if names.is_empty() {
                statements.push(
                    self.db
                        .prepare("DELETE FROM rule_provider_snapshots WHERE source_id = ?1")
                        .bind_refs(&[D1Type::Text(&source_id)])?,
                );
            } else {
                let placeholders = (2..=names.len() + 1)
                    .map(|index| format!("?{index}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut bindings: Vec<D1Type> = vec![D1Type::Text(&source_id)];
                bindings.extend(names.iter().map(|name| D1Type::Text(name)));
                statements.push(
                    self.db
                        .prepare(format!(
                            "DELETE FROM rule_provider_snapshots \
                             WHERE source_id = ?1 AND name NOT IN ({placeholders})"
                        ))
                        .bind_refs(&bindings)?,
                );
            }

            for (ordinal, provider) in providers.iter().enumerate() {
                statements.push(
                    self.db
                        .prepare(
                            "INSERT INTO source_rule_providers \
                             (source_id, ordinal, name, provider_type, behavior, url, path, interval, options) \
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        )
                        .bind_refs(&[
                            D1Type::Text(&source_id),
                            D1Type::Integer(ordinal as i32),
                            D1Type::Text(&provider.name),
                            option_text(provider.provider_type.as_deref()),
                            option_text(provider.behavior.as_deref()),
                            option_text(provider.url.as_deref()),
                            option_text(provider.path.as_deref()),
                            option_number(provider.interval),
                            D1Type::Text(&provider.options_json),
                        ])?,
                );
            }

            for chunk in statements.chunks(BATCH_SIZE) {
                self.db.batch(chunk.to_vec()).await?;
            }
            Ok(())
        })
        .await
    }

    async fn list_providers(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<ExtractedRuleProvider>, RepositoryError> {
        let source_id = source_id.to_string();
        SendFuture::new(async move {
            let result = self
                .db
                .prepare(format!(
                    "SELECT {PROVIDER_COLUMNS} FROM source_rule_providers \
                     WHERE source_id = ?1 ORDER BY ordinal"
                ))
                .bind_refs(&[D1Type::Text(&source_id)])?
                .all()
                .await?;
            Ok(result
                .results::<ProviderRow>()?
                .into_iter()
                .map(|row| ExtractedRuleProvider {
                    name: row.name,
                    provider_type: row.provider_type,
                    behavior: row.behavior,
                    url: row.url,
                    path: row.path,
                    interval: row.interval,
                    options_json: row.options,
                })
                .collect())
        })
        .await
    }

    async fn find_snapshot(
        &self,
        source_id: &SourceId,
        name: &str,
    ) -> Result<Option<RuleProviderSnapshot>, RepositoryError> {
        let source_id = source_id.to_string();
        let name = name.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {SNAPSHOT_COLUMNS} FROM rule_provider_snapshots \
                     WHERE source_id = ?1 AND name = ?2"
                ))
                .bind_refs(&[D1Type::Text(&source_id), D1Type::Text(&name)])?
                .first::<SnapshotRow>(None)
                .await?;
            row.map(row_to_snapshot).transpose()
        })
        .await
    }

    async fn list_snapshots(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<RuleProviderSnapshot>, RepositoryError> {
        let source_id = source_id.to_string();
        SendFuture::new(async move {
            let result = self
                .db
                .prepare(format!(
                    "SELECT {SNAPSHOT_COLUMNS} FROM rule_provider_snapshots \
                     WHERE source_id = ?1 ORDER BY name"
                ))
                .bind_refs(&[D1Type::Text(&source_id)])?
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

    async fn save_snapshot(&self, snapshot: &RuleProviderSnapshot) -> Result<(), RepositoryError> {
        let source_id = snapshot.source_id().to_string();
        let name = snapshot.name().to_string();
        let body = snapshot.body().to_vec();
        let etag = snapshot.etag().map(str::to_string);
        let last_modified = snapshot.last_modified().map(str::to_string);
        let fetched_at = snapshot.fetched_at() as f64;
        let body_hash = snapshot.body_hash().to_string();
        let rule_count = snapshot.rule_count() as i32;
        let last_error = snapshot.last_error().map(str::to_string);
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO rule_provider_snapshots \
                     (source_id, name, body, etag, last_modified, fetched_at, body_hash, rule_count, last_error) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                     ON CONFLICT(source_id, name) DO UPDATE SET \
                       body = excluded.body, \
                       etag = excluded.etag, \
                       last_modified = excluded.last_modified, \
                       fetched_at = excluded.fetched_at, \
                       body_hash = excluded.body_hash, \
                       rule_count = excluded.rule_count, \
                       last_error = excluded.last_error",
                )
                .bind_refs(&[
                    D1Type::Text(&source_id),
                    D1Type::Text(&name),
                    D1Type::Blob(&body),
                    option_text(etag.as_deref()),
                    option_text(last_modified.as_deref()),
                    D1Type::Real(fetched_at),
                    D1Type::Text(&body_hash),
                    D1Type::Integer(rule_count),
                    option_text(last_error.as_deref()),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn touch_snapshot(
        &self,
        source_id: &SourceId,
        name: &str,
        fetched_at: i64,
    ) -> Result<(), RepositoryError> {
        let source_id = source_id.to_string();
        let name = name.to_string();
        let fetched_at = fetched_at as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "UPDATE rule_provider_snapshots SET fetched_at = ?1, last_error = NULL \
                     WHERE source_id = ?2 AND name = ?3",
                )
                .bind_refs(&[
                    D1Type::Real(fetched_at),
                    D1Type::Text(&source_id),
                    D1Type::Text(&name),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn record_snapshot_error(
        &self,
        source_id: &SourceId,
        name: &str,
        error: &str,
    ) -> Result<(), RepositoryError> {
        let source_id = source_id.to_string();
        let name = name.to_string();
        let error = error.to_string();
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO rule_provider_snapshots (source_id, name, last_error) \
                     VALUES (?1, ?2, ?3) \
                     ON CONFLICT(source_id, name) DO UPDATE SET last_error = excluded.last_error",
                )
                .bind_refs(&[
                    D1Type::Text(&source_id),
                    D1Type::Text(&name),
                    D1Type::Text(&error),
                ])?
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
                        .prepare("DELETE FROM source_rule_providers WHERE source_id = ?1")
                        .bind_refs(&[D1Type::Text(&source_id)])?,
                    self.db
                        .prepare("DELETE FROM rule_provider_snapshots WHERE source_id = ?1")
                        .bind_refs(&[D1Type::Text(&source_id)])?,
                ])
                .await?;
            Ok(())
        })
        .await
    }
}
