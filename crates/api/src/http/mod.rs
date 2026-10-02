mod dto;
mod error;
mod handlers;
mod state;

pub use state::AppState;

use axum::routing::{get, post};
use axum::Router;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/auth/register", post(handlers::register))
        .route("/api/auth/login", post(handlers::login))
        .route("/api/auth/refresh", post(handlers::refresh))
        .route("/api/auth/logout", post(handlers::logout))
        .route("/api/users/me", get(handlers::me))
        .with_state(state)
}
