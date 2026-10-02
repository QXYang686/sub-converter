use async_trait::async_trait;
use serde::Deserialize;
use user::UserId;
use worker::d1::{D1Database, D1Type};
use worker::send::SendFuture;

use crate::domain::{Challenge, ChallengeKind, ChallengeRepository, RepositoryError};

pub struct D1ChallengeRepository {
    db: D1Database,
}

impl D1ChallengeRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct ChallengeRow {
    challenge: String,
    user_id: Option<String>,
    kind: String,
    created_at: i64,
    expires_at: i64,
}

fn row_to_challenge(row: ChallengeRow) -> Result<Challenge, RepositoryError> {
    let invalid = |message: String| RepositoryError::Unavailable(message);
    let user_id = row
        .user_id
        .map(|value| UserId::parse(&value).map_err(|err| invalid(err.to_string())))
        .transpose()?;
    let kind = ChallengeKind::parse(&row.kind)
        .ok_or_else(|| invalid(format!("unknown challenge kind: {}", row.kind)))?;
    Ok(Challenge::new(
        row.challenge,
        user_id,
        kind,
        row.created_at,
        row.expires_at - row.created_at,
    ))
}

#[async_trait]
impl ChallengeRepository for D1ChallengeRepository {
    async fn save(&self, challenge: &Challenge) -> Result<(), RepositoryError> {
        let value = challenge.challenge().to_string();
        let user_id = challenge.user_id().map(|id| id.to_string());
        let kind = challenge.kind().as_str().to_string();
        let created_at = challenge.created_at() as f64;
        let expires_at = challenge.expires_at() as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT OR REPLACE INTO webauthn_challenges \
                     (challenge, user_id, kind, created_at, expires_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind_refs(&[
                    D1Type::Text(&value),
                    match user_id.as_deref() {
                        Some(user_id) => D1Type::Text(user_id),
                        None => D1Type::Null,
                    },
                    D1Type::Text(&kind),
                    D1Type::Real(created_at),
                    D1Type::Real(expires_at),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }

    async fn consume(&self, challenge: &str) -> Result<Option<Challenge>, RepositoryError> {
        let value = challenge.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(
                    "SELECT challenge, user_id, kind, created_at, expires_at \
                     FROM webauthn_challenges WHERE challenge = ?1",
                )
                .bind_refs(&[D1Type::Text(&value)])?
                .first::<ChallengeRow>(None)
                .await?;

            let Some(row) = row else {
                return Ok(None);
            };

            self.db
                .prepare("DELETE FROM webauthn_challenges WHERE challenge = ?1")
                .bind_refs(&[D1Type::Text(&value)])?
                .run()
                .await?;

            row_to_challenge(row).map(Some)
        })
        .await
    }
}
