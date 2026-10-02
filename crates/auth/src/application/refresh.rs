use std::sync::Arc;

use crate::application::error::AppError;
use crate::application::ports::{Clock, RefreshTokenRepository, TokenService};
use user::UserRepository;

use super::UserView;

#[derive(Debug, Clone)]
pub struct RefreshCommand {
    pub refresh_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshResult {
    pub user: UserView,
    pub access_token: String,
    pub access_token_expires_at: i64,
    pub refresh_token: String,
    pub refresh_token_expires_at: i64,
}

pub struct RefreshHandler {
    users: Arc<dyn UserRepository>,
    token_service: Arc<dyn TokenService>,
    refresh_tokens: Arc<dyn RefreshTokenRepository>,
    clock: Arc<dyn Clock>,
}

impl RefreshHandler {
    pub fn new(
        users: Arc<dyn UserRepository>,
        token_service: Arc<dyn TokenService>,
        refresh_tokens: Arc<dyn RefreshTokenRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            token_service,
            refresh_tokens,
            clock,
        }
    }

    pub async fn handle(&self, command: RefreshCommand) -> Result<RefreshResult, AppError> {
        let now = self.clock.now();
        let token_hash = self.token_service.hash_refresh_token(&command.refresh_token);
        let stored = self
            .refresh_tokens
            .find_by_hash(&token_hash)
            .await?
            .ok_or(AppError::InvalidToken)?;

        if stored.revoked_at.is_some() {
            self.refresh_tokens
                .revoke_all_for_user(&stored.user_id, now)
                .await?;
            return Err(AppError::InvalidToken);
        }
        if stored.expires_at <= now {
            return Err(AppError::InvalidToken);
        }

        let user = self
            .users
            .find_by_id(&stored.user_id)
            .await?
            .ok_or(AppError::InvalidToken)?;

        self.refresh_tokens.revoke(stored.id, now).await?;
        let access = self.token_service.issue_access_token(user.id(), now).await?;
        let refresh = self.token_service.issue_refresh_token(user.id(), now).await?;
        self.refresh_tokens.save(&refresh.record).await?;

        Ok(RefreshResult {
            user: UserView::from(&user),
            access_token: access.token,
            access_token_expires_at: access.expires_at,
            refresh_token: refresh.raw,
            refresh_token_expires_at: refresh.record.expires_at,
        })
    }
}
