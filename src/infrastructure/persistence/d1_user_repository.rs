use async_trait::async_trait;
use serde::Deserialize;
use worker::d1::{D1Database, D1Type};
use worker::send::SendFuture;

use crate::domain::user::{
    PasswordHash, RepositoryError, User, UserId, UserRepository, Username,
};

const USER_COLUMNS: &str = "id, username, password_hash, created_at, updated_at";

pub struct D1UserRepository {
    db: D1Database,
}

impl D1UserRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
struct UserRow {
    id: String,
    username: String,
    password_hash: String,
    created_at: i64,
    updated_at: i64,
}

fn row_to_user(row: UserRow) -> Result<User, RepositoryError> {
    let invalid = |message: String| RepositoryError::Unavailable(message);
    let id = UserId::parse(&row.id).map_err(|err| invalid(err.to_string()))?;
    let username = Username::new(&row.username).map_err(|err| invalid(err.to_string()))?;
    let password_hash =
        PasswordHash::new(row.password_hash).map_err(|err| invalid(err.to_string()))?;
    Ok(User::restore(
        id,
        username,
        password_hash,
        row.created_at,
        row.updated_at,
    ))
}

impl From<worker::Error> for RepositoryError {
    fn from(err: worker::Error) -> Self {
        let message = err.to_string();
        if message.contains("UNIQUE constraint failed") {
            RepositoryError::UsernameConflict
        } else {
            RepositoryError::Unavailable(message)
        }
    }
}

#[async_trait]
impl UserRepository for D1UserRepository {
    async fn find_by_username(&self, username: &Username) -> Result<Option<User>, RepositoryError> {
        let username = username.value().to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!(
                    "SELECT {USER_COLUMNS} FROM users WHERE username = ?1"
                ))
                .bind_refs(&[D1Type::Text(&username)])?
                .first::<UserRow>(None)
                .await?;
            row.map(row_to_user).transpose()
        })
        .await
    }

    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, RepositoryError> {
        let id = id.to_string();
        SendFuture::new(async move {
            let row = self
                .db
                .prepare(format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?1"))
                .bind_refs(&[D1Type::Text(&id)])?
                .first::<UserRow>(None)
                .await?;
            row.map(row_to_user).transpose()
        })
        .await
    }

    async fn save(&self, user: &User) -> Result<(), RepositoryError> {
        let id = user.id().to_string();
        let username = user.username().value().to_string();
        let password_hash = user.password_hash().as_str().to_string();
        let created_at = user.created_at() as f64;
        let updated_at = user.updated_at() as f64;
        SendFuture::new(async move {
            self.db
                .prepare(
                    "INSERT INTO users (id, username, password_hash, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&username),
                    D1Type::Text(&password_hash),
                    D1Type::Real(created_at),
                    D1Type::Real(updated_at),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
    }
}
