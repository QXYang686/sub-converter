use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;
use worker::d1::{D1Database, D1Type};
use worker::send::SendFuture;

use crate::application::{PortError, RefreshTokenRepository, StoredRefreshToken};
use crate::domain::user::UserId;

const TOKEN_COLUMNS: &str = "id, user_id, token_hash, created_at, expires_at, revoked_at";

pub struct D1RefreshTokenRepository {
    db: D1Database,
}

impl D1RefreshTokenRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct RefreshTokenRow {
    id: String,
    user_id: String,
    token_hash: String,
    created_at: i64,
    expires_at: i64,
    revoked_at: Option<i64>,
}

fn row_to_token(row: RefreshTokenRow) -> Result<StoredRefreshToken, PortError> {
    let invalid = |message: String| PortError::Failure(message);
    Ok(StoredRefreshToken {
        id: Uuid::parse_str(&row.id).map_err(|err| invalid(err.to_string()))?,
        user_id: UserId::parse(&row.user_id).map_err(|err| invalid(err.to_string()))?,
        token_hash: row.token_hash,
        created_at: row.created_at,
        expires_at: row.expires_at,
        revoked_at: row.revoked_at,
    })
}

impl From<worker::Error> for PortError {
    fn from(err: worker::Error) -> Self {
        PortError::Failure(err.to_string())
    }
}

#[async_trait]
impl RefreshTokenRepository for D1RefreshTokenRepository {
    async fn save(&self, token: &StoredRefreshToken) -> Result<(), PortError> {
        let id = token.id.to_string();
        let user_id = token.user_id.to_string();
        let token_hash = token.token_hash.clone();
        let created_at = token.created_at as f64;
        let expires_at = token.expires_at as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO refresh_tokens (id, user_id, token_hash, created_at, expires_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&user_id),
                    D1Type::Text(&token_hash),
                    D1Type::Real(created_at),
                    D1Type::Real(expires_at),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<StoredRefreshToken>, PortError> {
        let token_hash = token_hash.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {TOKEN_COLUMNS} FROM refresh_tokens WHERE token_hash = ?1"
                ))
                .bind_refs(&[D1Type::Text(&token_hash)])?
                .first::<RefreshTokenRow>(None)
                .await?;
            row.map(row_to_token).transpose()
        })
        .await
    }

    async fn revoke(&self, id: Uuid, now: i64) -> Result<(), PortError> {
        let id = id.to_string();
        let now = now as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "UPDATE refresh_tokens SET revoked_at = ?1 \
                     WHERE id = ?2 AND revoked_at IS NULL",
                )
                .bind_refs(&[D1Type::Real(now), D1Type::Text(&id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn revoke_all_for_user(&self, user_id: &UserId, now: i64) -> Result<(), PortError> {
        let user_id = user_id.to_string();
        let now = now as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "UPDATE refresh_tokens SET revoked_at = ?1 \
                     WHERE user_id = ?2 AND revoked_at IS NULL",
                )
                .bind_refs(&[D1Type::Real(now), D1Type::Text(&user_id)])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }
}
