use std::sync::Arc;

use crate::application::{Clock, PasswordHasher, RefreshTokenRepository, TokenService};
use crate::domain::user::UserRepository;

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<dyn UserRepository>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub token_service: Arc<dyn TokenService>,
    pub refresh_tokens: Arc<dyn RefreshTokenRepository>,
    pub clock: Arc<dyn Clock>,
}
