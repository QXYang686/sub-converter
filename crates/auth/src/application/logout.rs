use std::sync::Arc;

use crate::application::error::AppError;
use crate::application::ports::{Clock, RefreshTokenRepository, TokenService};

#[derive(Debug, Clone)]
pub struct LogoutCommand {
    pub refresh_token: String,
}

pub struct LogoutHandler {
    token_service: Arc<dyn TokenService>,
    refresh_tokens: Arc<dyn RefreshTokenRepository>,
    clock: Arc<dyn Clock>,
}

impl LogoutHandler {
    pub fn new(
        token_service: Arc<dyn TokenService>,
        refresh_tokens: Arc<dyn RefreshTokenRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            token_service,
            refresh_tokens,
            clock,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: LogoutCommand) -> Result<(), AppError> {
        let token_hash = self.token_service.hash_refresh_token(&command.refresh_token);
        if let Some(stored) = self.refresh_tokens.find_by_hash(&token_hash).await? {
            if stored.revoked_at.is_none() {
                self.refresh_tokens.revoke(stored.id, self.clock.now()).await?;
            }
        }
        Ok(())
    }
}
