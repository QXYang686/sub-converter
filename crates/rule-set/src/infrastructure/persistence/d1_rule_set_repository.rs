use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use user::UserId;
use worker::d1::{D1DatabaseSession, D1Type};
use worker::send::SendFuture;

use crate::domain::{
    RepositoryError, RuleSet, RuleSetCategory, RuleSetContent, RuleSetContentFormat, RuleSetId,
    RuleSetInterval, RuleSetName, RuleSetPath, RuleSetRepository, RuleSetSource, RuleSetUrl,
};

const RULE_SET_COLUMNS: &str = "id, user_id, name, source_kind, url, path, category, \
     content_format, interval, enabled, created_at, updated_at";

const CONTENT_COLUMNS: &str = "rule_set_id, body, etag, last_modified, updated_at, body_hash, \
     rule_count, last_error, pinned";

pub struct D1RuleSetRepository {
    db: Arc<D1DatabaseSession>,
}

impl D1RuleSetRepository {
    pub fn new(db: Arc<D1DatabaseSession>) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct RuleSetRow {
    id: String,
    user_id: String,
    name: String,
    source_kind: String,
    url: Option<String>,
    path: Option<String>,
    category: Option<String>,
    content_format: String,
    interval: Option<i64>,
    enabled: i64,
    created_at: i64,
    updated_at: i64,
}

#[derive(Deserialize)]
struct ContentRow {
    rule_set_id: String,
    #[serde(default)]
    body: Option<serde_bytes::ByteBuf>,
    etag: Option<String>,
    last_modified: Option<String>,
    updated_at: i64,
    body_hash: Option<String>,
    rule_count: Option<i64>,
    last_error: Option<String>,
    pinned: Option<i64>,
}

fn invalid(message: String) -> RepositoryError {
    RepositoryError::Unavailable(message)
}

fn parse_source(
    kind: &str,
    url: Option<String>,
    path: Option<String>,
) -> Result<RuleSetSource, RepositoryError> {
    match kind {
        "remote" => {
            let url = url.ok_or_else(|| invalid("remote rule set missing url".to_string()))?;
            Ok(RuleSetSource::Remote(
                RuleSetUrl::new(&url).map_err(|err| invalid(err.to_string()))?,
            ))
        }
        "inline" => Ok(RuleSetSource::Inline),
        "local" => {
            let path = path.ok_or_else(|| invalid("local rule set missing path".to_string()))?;
            Ok(RuleSetSource::Local(
                RuleSetPath::new(&path).map_err(|err| invalid(err.to_string()))?,
            ))
        }
        other => Err(invalid(format!("unknown rule set source kind: {other}"))),
    }
}

fn row_to_rule_set(row: RuleSetRow) -> Result<RuleSet, RepositoryError> {
    let id = RuleSetId::parse(&row.id).map_err(|err| invalid(err.to_string()))?;
    let user_id = UserId::parse(&row.user_id).map_err(|err| invalid(err.to_string()))?;
    let name = RuleSetName::new(&row.name).map_err(|err| invalid(err.to_string()))?;
    let source = parse_source(&row.source_kind, row.url, row.path)?;
    let category = row
        .category
        .as_deref()
        .map(|raw| RuleSetCategory::parse(raw).ok_or_else(|| invalid(format!("bad category: {raw}"))))
        .transpose()?;
    let content_format = RuleSetContentFormat::parse(&row.content_format)
        .ok_or_else(|| invalid(format!("bad content format: {}", row.content_format)))?;
    let interval = row
        .interval
        .map(|seconds| RuleSetInterval::new(seconds).map_err(|err| invalid(err.to_string())))
        .transpose()?;

    Ok(RuleSet::restore(
        id,
        user_id,
        name,
        source,
        category,
        content_format,
        interval,
        row.enabled != 0,
        row.created_at,
        row.updated_at,
    ))
}

fn row_to_content(row: ContentRow) -> Result<RuleSetContent, RepositoryError> {
    let rule_set_id =
        RuleSetId::parse(&row.rule_set_id).map_err(|err| invalid(err.to_string()))?;
    Ok(RuleSetContent::restore(
        rule_set_id,
        row.body.map(serde_bytes::ByteBuf::into_vec),
        row.etag,
        row.last_modified,
        row.updated_at,
        row.body_hash.unwrap_or_default(),
        row.rule_count.unwrap_or(0).max(0) as u32,
        row.last_error,
        row.pinned.unwrap_or(0) != 0,
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
impl RuleSetRepository for D1RuleSetRepository {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &RuleSetId,
    ) -> Result<Option<RuleSet>, RepositoryError> {
        let id = id.to_string();
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {RULE_SET_COLUMNS} FROM rule_sets WHERE id = ?1 AND user_id = ?2"
                ))
                .bind_refs(&[D1Type::Text(&id), D1Type::Text(&user_id)])?
                .first::<RuleSetRow>(None)
                .await?;
            row.map(row_to_rule_set).transpose()
        })
        .await
    }

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<RuleSet>, RepositoryError> {
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            let result = self
                .db
                .prepare(format!(
                    "SELECT {RULE_SET_COLUMNS} FROM rule_sets WHERE user_id = ?1 ORDER BY name"
                ))
                .bind_refs(&[D1Type::Text(&user_id)])?
                .all()
                .await?;
            result
                .results::<RuleSetRow>()?
                .into_iter()
                .map(row_to_rule_set)
                .collect()
        })
        .await
    }

    async fn save(&self, rule_set: &RuleSet) -> Result<(), RepositoryError> {
        let id = rule_set.id().to_string();
        let user_id = rule_set.user_id().to_string();
        let name = rule_set.name().value().to_string();
        let source_kind = rule_set.source().kind().to_string();
        let url = rule_set.source().url().map(|url| url.value().to_string());
        let path = rule_set.source().path().map(|path| path.value().to_string());
        let category = rule_set.category().map(|c| c.as_str().to_string());
        let content_format = rule_set.content_format().as_str().to_string();
        let interval = rule_set.interval().map(|interval| interval.seconds());
        let enabled = if rule_set.enabled() { 1 } else { 0 };
        let created_at = rule_set.created_at();
        let updated_at = rule_set.updated_at();
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO rule_sets \
                     (id, user_id, name, source_kind, url, path, category, content_format, \
                      interval, enabled, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                )
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                    D1Type::Text(&name),
                    D1Type::Text(&source_kind),
                    option_text(url.as_deref()),
                    option_text(path.as_deref()),
                    option_text(category.as_deref()),
                    D1Type::Text(&content_format),
                    option_number(interval),
                    D1Type::Integer(enabled),
                    D1Type::Real(created_at as f64),
                    D1Type::Real(updated_at as f64),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn update(&self, rule_set: &RuleSet) -> Result<(), RepositoryError> {
        let id = rule_set.id().to_string();
        let user_id = rule_set.user_id().to_string();
        let name = rule_set.name().value().to_string();
        let source_kind = rule_set.source().kind().to_string();
        let url = rule_set.source().url().map(|url| url.value().to_string());
        let path = rule_set.source().path().map(|path| path.value().to_string());
        let category = rule_set.category().map(|c| c.as_str().to_string());
        let content_format = rule_set.content_format().as_str().to_string();
        let interval = rule_set.interval().map(|interval| interval.seconds());
        let enabled = if rule_set.enabled() { 1 } else { 0 };
        let updated_at = rule_set.updated_at();
        SendFuture::new(async move {
            self.db
                .prepare(
                    "UPDATE rule_sets SET \
                       name = ?1, source_kind = ?2, url = ?3, path = ?4, category = ?5, \
                       content_format = ?6, interval = ?7, enabled = ?8, updated_at = ?9 \
                     WHERE id = ?10 AND user_id = ?11",
                )
                .bind_refs(&[
                    D1Type::Text(&name),
                    D1Type::Text(&source_kind),
                    option_text(url.as_deref()),
                    option_text(path.as_deref()),
                    option_text(category.as_deref()),
                    D1Type::Text(&content_format),
                    option_number(interval),
                    D1Type::Integer(enabled),
                    D1Type::Real(updated_at as f64),
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, user_id: &UserId, id: &RuleSetId) -> Result<(), RepositoryError> {
        let id = id.to_string();
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            self.db
                .prepare("DELETE FROM rule_sets WHERE id = ?1 AND user_id = ?2")
                .bind_refs(&[D1Type::Text(&id), D1Type::Text(&user_id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn find_content(
        &self,
        id: &RuleSetId,
    ) -> Result<Option<RuleSetContent>, RepositoryError> {
        let id = id.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {CONTENT_COLUMNS} FROM rule_set_contents WHERE rule_set_id = ?1"
                ))
                .bind_refs(&[D1Type::Text(&id)])?
                .first::<ContentRow>(None)
                .await?;
            row.map(row_to_content).transpose()
        })
        .await
    }

    async fn save_content(&self, content: &RuleSetContent) -> Result<(), RepositoryError> {
        let rule_set_id = content.rule_set_id().to_string();
        let body = content.body().map(<[u8]>::to_vec);
        let etag = content.etag().map(str::to_string);
        let last_modified = content.last_modified().map(str::to_string);
        let updated_at = content.updated_at();
        let body_hash = content.body_hash().to_string();
        let rule_count = content.rule_count() as i32;
        let last_error = content.last_error().map(str::to_string);
        let pinned = if content.pinned() { 1 } else { 0 };
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO rule_set_contents \
                     (rule_set_id, body, etag, last_modified, updated_at, body_hash, rule_count, \
                      last_error, pinned) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                     ON CONFLICT(rule_set_id) DO UPDATE SET \
                       body = excluded.body, \
                       etag = excluded.etag, \
                       last_modified = excluded.last_modified, \
                       updated_at = excluded.updated_at, \
                       body_hash = excluded.body_hash, \
                       rule_count = excluded.rule_count, \
                       last_error = excluded.last_error, \
                       pinned = excluded.pinned",
                )
                .bind_refs(&[
                    D1Type::Text(&rule_set_id),
                    match body.as_deref() {
                        Some(body) => D1Type::Blob(body),
                        None => D1Type::Null,
                    },
                    option_text(etag.as_deref()),
                    option_text(last_modified.as_deref()),
                    D1Type::Real(updated_at as f64),
                    D1Type::Text(&body_hash),
                    D1Type::Integer(rule_count),
                    option_text(last_error.as_deref()),
                    D1Type::Integer(pinned),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn delete_content(&self, id: &RuleSetId) -> Result<(), RepositoryError> {
        let id = id.to_string();
        SendFuture::new(async move {
            self.db
                .prepare("DELETE FROM rule_set_contents WHERE rule_set_id = ?1")
                .bind_refs(&[D1Type::Text(&id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }
}
