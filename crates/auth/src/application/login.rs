use std::sync::Arc;

use user::{UserRepository, Username};

use crate::domain::PasswordCredentialRepository;

use super::dto::UserView;
use super::error::AppError;
use super::ports::{Clock, PasswordHasher, RefreshTokenRepository, TokenService};

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
    credentials: Arc<dyn PasswordCredentialRepository>,
    password_hasher: Arc<dyn PasswordHasher>,
    token_service: Arc<dyn TokenService>,
    refresh_tokens: Arc<dyn RefreshTokenRepository>,
    clock: Arc<dyn Clock>,
}

impl LoginHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        users: Arc<dyn UserRepository>,
        credentials: Arc<dyn PasswordCredentialRepository>,
        password_hasher: Arc<dyn PasswordHasher>,
        token_service: Arc<dyn TokenService>,
        refresh_tokens: Arc<dyn RefreshTokenRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            credentials,
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

        let credential = self
            .credentials
            .find_by_user_id(user.id())
            .await?
            .ok_or(AppError::InvalidCredentials)?;

        let password_matches = self
            .password_hasher
            .verify(&command.password, credential.password_hash())
            .await?;
        if !password_matches {
            return Err(AppError::InvalidCredentials);
        }

        let now = self.clock.now();
        let _ = self.credentials.mark_used(credential.id(), now).await;

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
