use std::sync::Arc;

use auth::application::{
    Clock, PasswordHasher, RefreshTokenRepository, TokenService, WebAuthnVerifier,
};
use auth::domain::{ChallengeRepository, CredentialRepository};
use subscription::application::{
    BackgroundTasks, Clock as SubscriptionClock, Fetcher, SecretGenerator,
};
use subscription::{
    PublicationRepository, PublicationSnapshotRepository, SnapshotRepository, SourceRepository,
};
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
    pub snapshots: Arc<dyn SnapshotRepository>,
    pub publication_snapshots: Arc<dyn PublicationSnapshotRepository>,
    pub fetcher: Arc<dyn Fetcher>,
    pub background: Arc<dyn BackgroundTasks>,
    pub subscription_clock: Arc<dyn SubscriptionClock>,
    pub secret_generator: Arc<dyn SecretGenerator>,
}
