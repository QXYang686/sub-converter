use std::sync::Arc;

use tower_service::Service;
use worker::*;

use auth::infrastructure::persistence::{
    D1ChallengeRepository, D1CredentialRepository, D1RefreshTokenRepository,
};
use auth::infrastructure::security::{
    JwtTokenService, OsRandomSource, Pbkdf2PasswordHasher, RustWebAuthnVerifier, SystemClock,
};
use subscription::infrastructure::fetch::HttpFetcher;
use subscription::infrastructure::persistence::{
    D1PublicationRepository, D1PublicationSnapshotRepository, D1RuleProviderRepository,
    D1SnapshotRepository, D1SourceRepository,
};
use subscription::infrastructure::security::{
    OsSecretGenerator, SystemClock as SubscriptionSystemClock,
};
use subscription::infrastructure::tasks::WorkerBackgroundTasks;
use user::infrastructure::persistence::D1UserRepository;

use crate::http::{router, AppState};
use crate::telemetry;

const DEFAULT_RP_ID: &str = "localhost";
const DEFAULT_ORIGINS: &str = "http://localhost:8080";
const RP_NAME: &str = "Sub Converter";

fn config_value(env: &Env, name: &str) -> Option<String> {
    env.var(name)
        .map(|value| value.to_string())
        .ok()
        .or_else(|| env.secret(name).map(|value| value.to_string()).ok())
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    telemetry::init(config_value(&env, "LOG_LEVEL"));
    let jwt_secret = env.secret("JWT_SECRET")?.to_string();
    let rp_id = config_value(&env, "WEBAUTHN_RP_ID").unwrap_or_else(|| DEFAULT_RP_ID.to_string());
    let origins =
        config_value(&env, "WEBAUTHN_ORIGINS").unwrap_or_else(|| DEFAULT_ORIGINS.to_string());

    let db = env.d1("DB")?;
    let session = Arc::new(db.with_session_constraint(D1SessionConstraint::FirstPrimary)?);
    let state = AppState {
        users: Arc::new(D1UserRepository::new(session.clone())),
        credentials: Arc::new(D1CredentialRepository::new(session.clone())),
        challenges: Arc::new(D1ChallengeRepository::new(session.clone())),
        password_hasher: Arc::new(Pbkdf2PasswordHasher::new()),
        token_service: Arc::new(JwtTokenService::new(jwt_secret)),
        refresh_tokens: Arc::new(D1RefreshTokenRepository::new(session.clone())),
        webauthn: Arc::new(RustWebAuthnVerifier::with_origins_csv(
            rp_id, RP_NAME, &origins,
        )),
        random: Arc::new(OsRandomSource),
        clock: Arc::new(SystemClock),
        sources: Arc::new(D1SourceRepository::new(session.clone())),
        publications: Arc::new(D1PublicationRepository::new(session.clone())),
        snapshots: Arc::new(D1SnapshotRepository::new(session.clone())),
        rule_providers: Arc::new(D1RuleProviderRepository::new(session.clone())),
        publication_snapshots: Arc::new(D1PublicationSnapshotRepository::new(session.clone())),
        fetcher: Arc::new(HttpFetcher),
        background: Arc::new(WorkerBackgroundTasks::new(ctx)),
        subscription_clock: Arc::new(SubscriptionSystemClock),
        secret_generator: Arc::new(OsSecretGenerator),
    };

    Ok(router(state).call(req).await?)
}
