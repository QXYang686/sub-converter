use std::sync::Arc;

use tower_service::Service;
use worker::*;

use auth::infrastructure::persistence::{
    D1PasswordCredentialRepository, D1RefreshTokenRepository,
};
use auth::infrastructure::security::{JwtTokenService, Pbkdf2PasswordHasher, SystemClock};
use user::infrastructure::persistence::D1UserRepository;

use crate::http::{router, AppState};

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    let jwt_secret = env.secret("JWT_SECRET")?.to_string();

    let state = AppState {
        users: Arc::new(D1UserRepository::new(env.d1("DB")?)),
        credentials: Arc::new(D1PasswordCredentialRepository::new(env.d1("DB")?)),
        password_hasher: Arc::new(Pbkdf2PasswordHasher::new()),
        token_service: Arc::new(JwtTokenService::new(jwt_secret)),
        refresh_tokens: Arc::new(D1RefreshTokenRepository::new(env.d1("DB")?)),
        clock: Arc::new(SystemClock),
    };

    Ok(router(state).call(req).await?)
}
