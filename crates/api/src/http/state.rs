use std::sync::Arc;

use auth::application::{Clock, PasswordHasher, RefreshTokenRepository, TokenService};
use auth::domain::PasswordCredentialRepository;
use user::UserRepository;

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<dyn UserRepository>,
    pub credentials: Arc<dyn PasswordCredentialRepository>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub token_service: Arc<dyn TokenService>,
    pub refresh_tokens: Arc<dyn RefreshTokenRepository>,
    pub clock: Arc<dyn Clock>,
}
