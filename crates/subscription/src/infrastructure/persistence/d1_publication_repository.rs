use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use user::UserId;

use worker::d1::{D1DatabaseSession, D1PreparedStatement, D1Type};
use worker::send::SendFuture;

use crate::domain::{
    Publication, PublicationId, PublicationRepository, PublicationSecret, PublicationSource,
    RepositoryError, SourceId, SubscriptionName,
};

const PUBLICATION_COLUMNS: &str =
    "id, user_id, name, secret, enabled, expires_at, created_at, updated_at";

pub struct D1PublicationRepository {
    db: Arc<D1DatabaseSession>,
}

impl D1PublicationRepository {
    pub fn new(db: Arc<D1DatabaseSession>) -> Self {
        Self { db }
    }

    fn binding_statements(
        &self,
        publication: &Publication,
    ) -> Result<Vec<D1PreparedStatement>, RepositoryError> {
        let publication_id = publication.id().to_string();
        publication
            .sources()
            .iter()
            .map(|source| {
                let source_id = source.source_id().to_string();
                Ok(self
                    .db
                    .prepare(
                        "INSERT INTO publication_sources (publication_id, source_id, position) \
                         VALUES (?1, ?2, ?3)",
                    )
                    .bind_refs(&[
                        D1Type::Text(&publication_id),
                        D1Type::Text(&source_id),
                        D1Type::Integer(source.position() as i32),
                    ])?)
            })
            .collect()
    }
}

#[derive(Deserialize)]
struct PublicationRow {
    id: String,
    user_id: String,
    name: String,
    secret: String,
    enabled: i64,
    expires_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Deserialize)]
struct BindingRow {
    publication_id: String,
    source_id: String,
    position: i64,
}

fn row_to_publication(
    row: PublicationRow,
    sources: Vec<PublicationSource>,
) -> Result<Publication, RepositoryError> {
    let invalid = |message: String| RepositoryError::Unavailable(message);
    let id = PublicationId::parse(&row.id).map_err(|err| invalid(err.to_string()))?;
    let user_id = UserId::parse(&row.user_id).map_err(|err| invalid(err.to_string()))?;
    let name = SubscriptionName::new(&row.name).map_err(|err| invalid(err.to_string()))?;
    let secret = PublicationSecret::new(&row.secret).map_err(|err| invalid(err.to_string()))?;
    Ok(Publication::restore(
        id,
        user_id,
        name,
        secret,
        row.enabled != 0,
        row.expires_at,
        row.created_at,
        row.updated_at,
        sources,
    ))
}

fn row_to_binding(row: BindingRow) -> Result<PublicationSource, RepositoryError> {
    let source_id = SourceId::parse(&row.source_id)
        .map_err(|err| RepositoryError::Unavailable(err.to_string()))?;
    Ok(PublicationSource::new(
        source_id,
        row.position.max(0) as u32,
    ))
}

#[async_trait]
impl PublicationRepository for D1PublicationRepository {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &PublicationId,
    ) -> Result<Option<Publication>, RepositoryError> {
        let id = id.to_string();
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {PUBLICATION_COLUMNS} FROM publications \
                     WHERE id = ?1 AND user_id = ?2"
                ))
                .bind_refs(&[D1Type::Text(&id), D1Type::Text(&user_id)])?
                .first::<PublicationRow>(None)
                .await?;
            let Some(row) = row else {
                return Ok(None);
            };

            let result = self
                .db
                .prepare(
                    "SELECT publication_id, source_id, position FROM publication_sources \
                     WHERE publication_id = ?1 ORDER BY position",
                )
                .bind_refs(&[D1Type::Text(&id)])?
                .all()
                .await?;
            let sources = result
                .results::<BindingRow>()?
                .into_iter()
                .map(row_to_binding)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(row_to_publication(row, sources)?))
        })
        .await
    }

    async fn find_by_secret(
        &self,
        secret: &PublicationSecret,
    ) -> Result<Option<Publication>, RepositoryError> {
        let secret = secret.value().to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {PUBLICATION_COLUMNS} FROM publications WHERE secret = ?1"
                ))
                .bind_refs(&[D1Type::Text(&secret)])?
                .first::<PublicationRow>(None)
                .await?;
            let Some(row) = row else {
                return Ok(None);
            };

            let id = row.id.clone();
            let result = self
                .db
                .prepare(
                    "SELECT publication_id, source_id, position FROM publication_sources \
                     WHERE publication_id = ?1 ORDER BY position",
                )
                .bind_refs(&[D1Type::Text(&id)])?
                .all()
                .await?;
            let sources = result
                .results::<BindingRow>()?
                .into_iter()
                .map(row_to_binding)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(row_to_publication(row, sources)?))
        })
        .await
    }

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Publication>, RepositoryError> {
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            let result = self
                .db
                .prepare(format!(
                    "SELECT {PUBLICATION_COLUMNS} FROM publications WHERE user_id = ?1 \
                     ORDER BY created_at DESC, id"
                ))
                .bind_refs(&[D1Type::Text(&user_id)])?
                .all()
                .await?;
            let rows = result.results::<PublicationRow>()?;

            let result = self
                .db
                .prepare(
                    "SELECT publication_sources.publication_id, publication_sources.source_id, \
                            publication_sources.position \
                     FROM publication_sources \
                     JOIN publications ON publications.id = publication_sources.publication_id \
                     WHERE publications.user_id = ?1 \
                     ORDER BY publication_sources.publication_id, publication_sources.position",
                )
                .bind_refs(&[D1Type::Text(&user_id)])?
                .all()
                .await?;
            let mut bindings: HashMap<String, Vec<PublicationSource>> = HashMap::new();
            for row in result.results::<BindingRow>()? {
                let publication_id = row.publication_id.clone();
                bindings
                    .entry(publication_id)
                    .or_default()
                    .push(row_to_binding(row)?);
            }

            rows.into_iter()
                .map(|row| {
                    let sources = bindings.remove(&row.id).unwrap_or_default();
                    row_to_publication(row, sources)
                })
                .collect()
        })
        .await
    }

    async fn save(&self, publication: &Publication) -> Result<(), RepositoryError> {
        let id = publication.id().to_string();
        let user_id = publication.user_id().to_string();
        let name = publication.name().value().to_string();
        let secret = publication.secret().value().to_string();
        let enabled = if publication.enabled() { 1 } else { 0 };
        let expires_at = publication.expires_at().map(|value| value as f64);
        let created_at = publication.created_at() as f64;
        let updated_at = publication.updated_at() as f64;
        let mut statements = self.binding_statements(publication)?;

        SendFuture::new(async move {
            let insert = self
                .db
                .prepare(
                    "INSERT INTO publications \
                     (id, user_id, name, secret, enabled, expires_at, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                    D1Type::Text(&name),
                    D1Type::Text(&secret),
                    D1Type::Integer(enabled),
                    option_real(expires_at),
                    D1Type::Real(created_at),
                    D1Type::Real(updated_at),
                ])?;
            let mut batch = Vec::with_capacity(statements.len() + 1);
            batch.push(insert);
            batch.append(&mut statements);
            self.db.batch(batch).await?;
            Ok(())
        })
        .await
    }

    async fn update(&self, publication: &Publication) -> Result<(), RepositoryError> {
        let id = publication.id().to_string();
        let user_id = publication.user_id().to_string();
        let name = publication.name().value().to_string();
        let secret = publication.secret().value().to_string();
        let enabled = if publication.enabled() { 1 } else { 0 };
        let expires_at = publication.expires_at().map(|value| value as f64);
        let updated_at = publication.updated_at() as f64;
        let mut statements = self.binding_statements(publication)?;

        SendFuture::new(async move {
            let update = self
                .db
                .prepare(
                    "UPDATE publications \
                     SET name = ?1, secret = ?2, enabled = ?3, expires_at = ?4, updated_at = ?5 \
                     WHERE id = ?6 AND user_id = ?7",
                )
                .bind_refs(&[
                    D1Type::Text(&name),
                    D1Type::Text(&secret),
                    D1Type::Integer(enabled),
                    option_real(expires_at),
                    D1Type::Real(updated_at),
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                ])?;
            let clear = self
                .db
                .prepare("DELETE FROM publication_sources WHERE publication_id = ?1")
                .bind_refs(&[D1Type::Text(&id)])?;

            let mut batch = Vec::with_capacity(statements.len() + 2);
            batch.push(update);
            batch.push(clear);
            batch.append(&mut statements);
            self.db.batch(batch).await?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, user_id: &UserId, id: &PublicationId) -> Result<(), RepositoryError> {
        let id = id.to_string();
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            self.db
                .batch(vec![
                    self.db
                        .prepare("DELETE FROM publication_sources WHERE publication_id = ?1")
                        .bind_refs(&[D1Type::Text(&id)])?,
                    self.db
                        .prepare("DELETE FROM publications WHERE id = ?1 AND user_id = ?2")
                        .bind_refs(&[D1Type::Text(&id), D1Type::Text(&user_id)])?,
                ])
                .await?;
            Ok(())
        })
        .await
    }
}

fn option_real(value: Option<f64>) -> D1Type<'static> {
    match value {
        Some(value) => D1Type::Real(value),
        None => D1Type::Null,
    }
}
