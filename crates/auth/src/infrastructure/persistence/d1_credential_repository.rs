use async_trait::async_trait;
use serde::Deserialize;
use user::UserId;
use worker::d1::{D1Database, D1Type};
use worker::send::SendFuture;

use crate::domain::{
    Credential, CredentialId, CredentialRepository, CredentialSecret, Passkey, PasswordHash,
    RepositoryError,
};

const CREDENTIAL_COLUMNS: &str = "id, user_id, kind, password_hash, credential_id, public_key, \
                                   sign_count, transports, label, created_at, last_used_at";

pub struct D1CredentialRepository {
    db: D1Database,
}

impl D1CredentialRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct CredentialRow {
    id: String,
    user_id: String,
    kind: String,
    password_hash: Option<String>,
    credential_id: Option<String>,
    public_key: Option<String>,
    sign_count: Option<i64>,
    transports: Option<String>,
    label: Option<String>,
    created_at: i64,
    last_used_at: Option<i64>,
}

fn row_to_credential(row: CredentialRow) -> Result<Credential, RepositoryError> {
    let invalid = |message: String| RepositoryError::Unavailable(message);
    let id = CredentialId::parse(&row.id).map_err(|err| invalid(err.to_string()))?;
    let user_id = UserId::parse(&row.user_id).map_err(|err| invalid(err.to_string()))?;

    let secret = match row.kind.as_str() {
        "password" => {
            let hash = row
                .password_hash
                .ok_or_else(|| invalid("password hash missing".to_string()))?;
            CredentialSecret::Password(
                PasswordHash::new(hash).map_err(|err| invalid(err.to_string()))?,
            )
        }
        "passkey" => {
            let credential_id = row
                .credential_id
                .ok_or_else(|| invalid("passkey credential id missing".to_string()))?;
            let public_key = row
                .public_key
                .ok_or_else(|| invalid("passkey public key missing".to_string()))?;
            CredentialSecret::Passkey(Passkey::new(
                credential_id,
                public_key,
                row.sign_count.unwrap_or(0) as u32,
                row.transports,
            ))
        }
        other => return Err(invalid(format!("unknown credential kind: {other}"))),
    };

    Ok(Credential::restore(
        id,
        user_id,
        row.label,
        row.created_at,
        row.last_used_at,
        secret,
    ))
}

impl From<worker::Error> for RepositoryError {
    fn from(err: worker::Error) -> Self {
        RepositoryError::Unavailable(err.to_string())
    }
}

#[async_trait]
impl CredentialRepository for D1CredentialRepository {
    async fn find_password_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<Credential>, RepositoryError> {
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

    async fn find_passkeys_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<Credential>, RepositoryError> {
        let user_id = user_id.to_string();
        SendFuture::new(async move {
            let result = self
                .db
                .prepare(format!(
                    "SELECT {CREDENTIAL_COLUMNS} FROM credentials \
                     WHERE user_id = ?1 AND kind = 'passkey' ORDER BY created_at"
                ))
                .bind_refs(&[D1Type::Text(&user_id)])?
                .all()
                .await?;
            result
                .results::<CredentialRow>()?
                .into_iter()
                .map(row_to_credential)
                .collect()
        })
        .await
    }

    async fn find_passkey_by_credential_id(
        &self,
        credential_id: &str,
    ) -> Result<Option<Credential>, RepositoryError> {
        let credential_id = credential_id.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {CREDENTIAL_COLUMNS} FROM credentials \
                     WHERE credential_id = ?1 AND kind = 'passkey'"
                ))
                .bind_refs(&[D1Type::Text(&credential_id)])?
                .first::<CredentialRow>(None)
                .await?;
            row.map(row_to_credential).transpose()
        })
        .await
    }

    async fn save(&self, credential: &Credential) -> Result<(), RepositoryError> {
        let id = credential.id().to_string();
        let user_id = credential.user_id().to_string();
        let label = credential.label().map(str::to_string);
        let created_at = credential.created_at() as f64;
        let (kind, password_hash, credential_id, public_key, sign_count, transports) =
            match credential.secret() {
                CredentialSecret::Password(hash) => {
                    ("password", Some(hash.as_str().to_string()), None, None, None, None)
                }
                CredentialSecret::Passkey(passkey) => (
                    "passkey",
                    None,
                    Some(passkey.credential_id().to_string()),
                    Some(passkey.public_key().to_string()),
                    Some(passkey.sign_count() as f64),
                    passkey.transports().map(str::to_string),
                ),
            };

        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO credentials \
                     (id, user_id, kind, password_hash, credential_id, public_key, sign_count, \
                      transports, label, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                )
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                    D1Type::Text(kind),
                    option_text(password_hash.as_deref()),
                    option_text(credential_id.as_deref()),
                    option_text(public_key.as_deref()),
                    option_real(sign_count),
                    option_text(transports.as_deref()),
                    option_text(label.as_deref()),
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

    async fn update_sign_count(
        &self,
        id: &CredentialId,
        sign_count: u32,
    ) -> Result<(), RepositoryError> {
        let id = id.to_string();
        let sign_count = sign_count as f64;
        SendFuture::new(async move {
            self.db
                .prepare("UPDATE credentials SET sign_count = ?1 WHERE id = ?2")
                .bind_refs(&[D1Type::Real(sign_count), D1Type::Text(&id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, id: &CredentialId) -> Result<(), RepositoryError> {
        let id = id.to_string();
        SendFuture::new(async move {
            self.db
                .prepare("DELETE FROM credentials WHERE id = ?1")
                .bind_refs(&[D1Type::Text(&id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }
}

fn option_text(value: Option<&str>) -> D1Type<'_> {
    match value {
        Some(value) => D1Type::Text(value),
        None => D1Type::Null,
    }
}

fn option_real(value: Option<f64>) -> D1Type<'static> {
    match value {
        Some(value) => D1Type::Real(value),
        None => D1Type::Null,
    }
}
