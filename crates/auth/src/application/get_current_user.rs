use std::sync::Arc;

use crate::application::error::AppError;
use crate::application::ports::{Clock, TokenService};
use user::UserRepository;

use super::UserView;

pub struct GetCurrentUserHandler {
    users: Arc<dyn UserRepository>,
    token_service: Arc<dyn TokenService>,
    clock: Arc<dyn Clock>,
}

impl GetCurrentUserHandler {
    pub fn new(
        users: Arc<dyn UserRepository>,
        token_service: Arc<dyn TokenService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            token_service,
            clock,
        }
    }

    pub async fn handle(&self, access_token: &str) -> Result<UserView, AppError> {
        let user_id = self
            .token_service
            .verify_access_token(access_token, self.clock.now())
            .await
            .map_err(|_| AppError::InvalidToken)?;

        let user = self
            .users
            .find_by_id(&user_id)
            .await?
            .ok_or(AppError::InvalidToken)?;

        Ok(UserView::from(&user))
    }
}
