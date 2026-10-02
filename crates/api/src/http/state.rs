use std::sync::Arc;

use auth::application::{Clock, PasswordHasher, RefreshTokenRepository, TokenService, WebAuthnVerifier};
use auth::domain::{ChallengeRepository, CredentialRepository};
use user::UserRepository;

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<dyn UserRepository>,
    pub credentials: Arc<dyn CredentialRepository>,
    pub challenges: Arc<dyn ChallengeRepository>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub token_service: Arc<dyn TokenService>,
    pub refresh_tokens: Arc<dyn RefreshTokenRepository>,
    pub webauthn: Arc<dyn WebAuthnVerifier>,
    pub random: Arc<dyn auth::application::RandomSource>,
    pub clock: Arc<dyn Clock>,
}
