use async_trait::async_trait;
use serde::Deserialize;
use user::UserId;
use worker::d1::{D1Database, D1Type};
use worker::send::SendFuture;

use crate::domain::{
    CredentialId, PasswordCredential, PasswordCredentialRepository, PasswordHash,
    RepositoryError,
};

const CREDENTIAL_COLUMNS: &str = "id, user_id, password_hash, created_at, last_used_at";

pub struct D1PasswordCredentialRepository {
    db: D1Database,
}

impl D1PasswordCredentialRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct CredentialRow {
    id: String,
    user_id: String,
    password_hash: String,
    created_at: i64,
    last_used_at: Option<i64>,
}

fn row_to_credential(row: CredentialRow) -> Result<PasswordCredential, RepositoryError> {
    let invalid = |message: String| RepositoryError::Unavailable(message);
    let id = CredentialId::parse(&row.id).map_err(|err| invalid(err.to_string()))?;
    let user_id = UserId::parse(&row.user_id).map_err(|err| invalid(err.to_string()))?;
    let password_hash = PasswordHash::new(row.password_hash).map_err(|err| invalid(err.to_string()))?;
    Ok(PasswordCredential::restore(
        id,
        user_id,
        password_hash,
        row.created_at,
        row.last_used_at,
    ))
}

impl From<worker::Error> for RepositoryError {
    fn from(err: worker::Error) -> Self {
        RepositoryError::Unavailable(err.to_string())
    }
}

#[async_trait]
impl PasswordCredentialRepository for D1PasswordCredentialRepository {
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<PasswordCredential>, RepositoryError> {
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {CREDENTIAL_COLUMNS} FROM credentials \
                     WHERE user_id = ?1 AND kind = 'password'"
                ))
                .bind_refs(&[D1Type::Text(&user_id)])?
                .first::<CredentialRow>(None)
                .await?;
            row.map(row_to_credential).transpose()
        })
        .await
    }

    async fn save(&self, credential: &PasswordCredential) -> Result<(), RepositoryError> {
        let id = credential.id().to_string();
        let user_id = credential.user_id().to_string();
        let password_hash = credential.password_hash().as_str().to_string();
        let created_at = credential.created_at() as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO credentials (id, user_id, kind, password_hash, created_at) \
                     VALUES (?1, ?2, 'password', ?3, ?4)",
                )
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                    D1Type::Text(&password_hash),
                    D1Type::Real(created_at),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn mark_used(&self, id: &CredentialId, now: i64) -> Result<(), RepositoryError> {
        let id = id.to_string();
        let now = now as f64;
        SendFuture::new(async move {
            self.db
                .prepare("UPDATE credentials SET last_used_at = ?1 WHERE id = ?2")
                .bind_refs(&[D1Type::Real(now), D1Type::Text(&id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }
}
