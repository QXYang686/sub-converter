use async_trait::async_trait;
use serde::Deserialize;
use user::UserId;
use worker::d1::{D1Database, D1Type};
use worker::send::SendFuture;

use crate::domain::{
    RepositoryError, Source, SourceId, SourceRepository, SourceUrl, SubscriptionName,
};

const SOURCE_COLUMNS: &str = "id, user_id, name, url, enabled, created_at, updated_at";

pub struct D1SourceRepository {
    db: D1Database,
}

impl D1SourceRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct SourceRow {
    id: String,
    user_id: String,
    name: String,
    url: String,
    enabled: i64,
    created_at: i64,
    updated_at: i64,
}

fn row_to_source(row: SourceRow) -> Result<Source, RepositoryError> {
    let invalid = |message: String| RepositoryError::Unavailable(message);
    let id = SourceId::parse(&row.id).map_err(|err| invalid(err.to_string()))?;
    let user_id = UserId::parse(&row.user_id).map_err(|err| invalid(err.to_string()))?;
    let name = SubscriptionName::new(&row.name).map_err(|err| invalid(err.to_string()))?;
    let url = SourceUrl::new(&row.url).map_err(|err| invalid(err.to_string()))?;
    Ok(Source::restore(
        id,
        user_id,
        name,
        url,
        row.enabled != 0,
        row.created_at,
        row.updated_at,
    ))
}

#[async_trait]
impl SourceRepository for D1SourceRepository {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &SourceId,
    ) -> Result<Option<Source>, RepositoryError> {
        let id = id.to_string();
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {SOURCE_COLUMNS} FROM sources WHERE id = ?1 AND user_id = ?2"
                ))
                .bind_refs(&[D1Type::Text(&id), D1Type::Text(&user_id)])?
                .first::<SourceRow>(None)
                .await?;
            row.map(row_to_source).transpose()
        })
        .await
    }

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Source>, RepositoryError> {
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            let result = self
                .db
                .prepare(format!(
                    "SELECT {SOURCE_COLUMNS} FROM sources WHERE user_id = ?1 \
                     ORDER BY created_at DESC, id"
                ))
                .bind_refs(&[D1Type::Text(&user_id)])?
                .all()
                .await?;
            result
                .results::<SourceRow>()?
                .into_iter()
                .map(row_to_source)
                .collect()
        })
        .await
    }

    async fn save(&self, source: &Source) -> Result<(), RepositoryError> {
        let id = source.id().to_string();
        let user_id = source.user_id().to_string();
        let name = source.name().value().to_string();
        let url = source.url().value().to_string();
        let enabled = if source.enabled() { 1 } else { 0 };
        let created_at = source.created_at() as f64;
        let updated_at = source.updated_at() as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO sources (id, user_id, name, url, enabled, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                    D1Type::Text(&name),
                    D1Type::Text(&url),
                    D1Type::Integer(enabled),
                    D1Type::Real(created_at),
                    D1Type::Real(updated_at),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn update(&self, source: &Source) -> Result<(), RepositoryError> {
        let id = source.id().to_string();
        let user_id = source.user_id().to_string();
        let name = source.name().value().to_string();
        let url = source.url().value().to_string();
        let enabled = if source.enabled() { 1 } else { 0 };
        let updated_at = source.updated_at() as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "UPDATE sources SET name = ?1, url = ?2, enabled = ?3, updated_at = ?4 \
                     WHERE id = ?5 AND user_id = ?6",
                )
                .bind_refs(&[
                    D1Type::Text(&name),
                    D1Type::Text(&url),
                    D1Type::Integer(enabled),
                    D1Type::Real(updated_at),
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, user_id: &UserId, id: &SourceId) -> Result<(), RepositoryError> {
        let id = id.to_string();
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            self.db
                .batch(vec![
                    self.db
                        .prepare("DELETE FROM publication_sources WHERE source_id = ?1")
                        .bind_refs(&[D1Type::Text(&id)])?,
                    self.db
                        .prepare("DELETE FROM sources WHERE id = ?1 AND user_id = ?2")
                        .bind_refs(&[D1Type::Text(&id), D1Type::Text(&user_id)])?,
                ])
                .await?;
            Ok(())
        })
        .await
    }
}
