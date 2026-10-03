use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use worker::d1::{D1DatabaseSession, D1Type};
use worker::send::SendFuture;

use crate::domain::{
    PublicationId, PublicationSnapshot, PublicationSnapshotRepository, RepositoryError,
    SubscriptionUserInfo,
};

pub struct D1PublicationSnapshotRepository {
    db: Arc<D1DatabaseSession>,
}

impl D1PublicationSnapshotRepository {
    pub fn new(db: Arc<D1DatabaseSession>) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct SnapshotRow {
    content: String,
    userinfo_upload: Option<i64>,
    userinfo_download: Option<i64>,
    userinfo_total: Option<i64>,
    userinfo_expire: Option<i64>,
    generated_at: i64,
}

fn option_number(value: Option<i64>) -> D1Type<'static> {
    match value {
        Some(value) => D1Type::Real(value as f64),
        None => D1Type::Null,
    }
}

#[async_trait]
impl PublicationSnapshotRepository for D1PublicationSnapshotRepository {
    async fn find(
        &self,
        publication_id: &PublicationId,
        format: &str,
    ) -> Result<Option<PublicationSnapshot>, RepositoryError> {
        let publication_id = publication_id.to_string();
        let format = format.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(
                    "SELECT content, userinfo_upload, userinfo_download, userinfo_total, \
                            userinfo_expire, generated_at \
                     FROM publication_snapshots WHERE publication_id = ?1 AND format = ?2",
                )
                .bind_refs(&[D1Type::Text(&publication_id), D1Type::Text(&format)])?
                .first::<SnapshotRow>(None)
                .await?;
            let Some(row) = row else {
                return Ok(None);
            };
            let publication_id = PublicationId::parse(&publication_id)
                .map_err(|err| RepositoryError::Unavailable(err.to_string()))?;
            Ok(Some(PublicationSnapshot::restore(
                publication_id,
                format,
                row.content,
                SubscriptionUserInfo {
                    upload: row.userinfo_upload,
                    download: row.userinfo_download,
                    total: row.userinfo_total,
                    expire: row.userinfo_expire,
                },
                row.generated_at,
            )))
        })
        .await
    }

    async fn save(&self, snapshot: &PublicationSnapshot) -> Result<(), RepositoryError> {
        let publication_id = snapshot.publication_id().to_string();
        let format = snapshot.format().to_string();
        let content = snapshot.content().to_string();
        let userinfo = snapshot.userinfo().clone();
        let generated_at = snapshot.generated_at() as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO publication_snapshots \
                     (publication_id, format, content, userinfo_upload, userinfo_download, \
                      userinfo_total, userinfo_expire, generated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                     ON CONFLICT(publication_id, format) DO UPDATE SET \
                       content = excluded.content, \
                       userinfo_upload = excluded.userinfo_upload, \
                       userinfo_download = excluded.userinfo_download, \
                       userinfo_total = excluded.userinfo_total, \
                       userinfo_expire = excluded.userinfo_expire, \
                       generated_at = excluded.generated_at",
                )
                .bind_refs(&[
                    D1Type::Text(&publication_id),
                    D1Type::Text(&format),
                    D1Type::Text(&content),
                    option_number(userinfo.upload),
                    option_number(userinfo.download),
                    option_number(userinfo.total),
                    option_number(userinfo.expire),
                    D1Type::Real(generated_at),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, publication_id: &PublicationId) -> Result<(), RepositoryError> {
        let publication_id = publication_id.to_string();
        SendFuture::new(async move {
            self.db
                .prepare("DELETE FROM publication_snapshots WHERE publication_id = ?1")
                .bind_refs(&[D1Type::Text(&publication_id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }
}
