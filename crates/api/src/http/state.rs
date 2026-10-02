use std::sync::Arc;

use auth::application::{
    Clock, PasswordHasher, RefreshTokenRepository, TokenService, WebAuthnVerifier,
};
use auth::domain::{ChallengeRepository, CredentialRepository};
use subscription::application::{Clock as SubscriptionClock, SecretGenerator};
use subscription::{PublicationRepository, SourceRepository};
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
    pub sources: Arc<dyn SourceRepository>,
    pub publications: Arc<dyn PublicationRepository>,
    pub subscription_clock: Arc<dyn SubscriptionClock>,
    pub secret_generator: Arc<dyn SecretGenerator>,
}
