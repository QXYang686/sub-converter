use std::sync::Arc;

use crate::application::error::AppError;
use crate::application::ports::{Clock, PasswordHasher, RefreshTokenRepository, TokenService};
use crate::domain::user::{UserRepository, Username};

use super::UserView;

#[derive(Debug, Clone)]
pub struct LoginCommand {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginResult {
    pub user: UserView,
    pub access_token: String,
    pub access_token_expires_at: i64,
    pub refresh_token: String,
    pub refresh_token_expires_at: i64,
}

pub struct LoginHandler {
    users: Arc<dyn UserRepository>,
    password_hasher: Arc<dyn PasswordHasher>,
    token_service: Arc<dyn TokenService>,
    refresh_tokens: Arc<dyn RefreshTokenRepository>,
    clock: Arc<dyn Clock>,
}

impl LoginHandler {
    pub fn new(
        users: Arc<dyn UserRepository>,
        password_hasher: Arc<dyn PasswordHasher>,
        token_service: Arc<dyn TokenService>,
        refresh_tokens: Arc<dyn RefreshTokenRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            password_hasher,
            token_service,
            refresh_tokens,
            clock,
        }
    }

    pub async fn handle(&self, command: LoginCommand) -> Result<LoginResult, AppError> {
        let username =
            Username::new(&command.username).map_err(|_| AppError::InvalidCredentials)?;
        let user = self
            .users
            .find_by_username(&username)
            .await?
            .ok_or(AppError::InvalidCredentials)?;

        let password_matches = self
            .password_hasher
            .verify(&command.password, user.password_hash())
            .await?;
        if !password_matches {
            return Err(AppError::InvalidCredentials);
        }

        let now = self.clock.now();
        let access = self.token_service.issue_access_token(user.id(), now).await?;
        let refresh = self.token_service.issue_refresh_token(user.id(), now).await?;
        self.refresh_tokens.save(&refresh.record).await?;

        Ok(LoginResult {
            user: UserView::from(&user),
            access_token: access.token,
            access_token_expires_at: access.expires_at,
            refresh_token: refresh.raw,
            refresh_token_expires_at: refresh.record.expires_at,
        })
    }
}
