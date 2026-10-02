mod dto;
mod error;
mod handlers;
mod session;
mod state;
mod trace;

pub use state::AppState;

use axum::routing::{delete, get, post, put};
use axum::Router;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/auth/register", post(handlers::register))
        .route("/api/auth/login", post(handlers::login))
        .route("/api/auth/refresh", post(handlers::refresh))
        .route("/api/auth/logout", post(handlers::logout))
        .route("/api/users/me", get(handlers::me))
        .route(
            "/api/auth/passkey/register/start",
            post(handlers::passkey_register_start),
        )
        .route(
            "/api/auth/passkey/register/finish",
            post(handlers::passkey_register_finish),
        )
        .route(
            "/api/auth/passkey/login/start",
            post(handlers::passkey_login_start),
        )
        .route(
            "/api/auth/passkey/login/finish",
            post(handlers::passkey_login_finish),
        )
        .route("/api/auth/passkeys", get(handlers::list_passkeys))
        .route("/api/auth/passkeys/{id}", delete(handlers::delete_passkey))
        .route(
            "/api/subscriptions/sources",
            get(handlers::list_sources).post(handlers::create_source),
        )
        .route(
            "/api/subscriptions/sources/{id}",
            get(handlers::get_source)
                .patch(handlers::update_source)
                .delete(handlers::delete_source),
        )
        .route(
            "/api/subscriptions/publications",
            get(handlers::list_publications).post(handlers::create_publication),
        )
        .route(
            "/api/subscriptions/publications/{id}",
            get(handlers::get_publication)
                .patch(handlers::update_publication)
                .delete(handlers::delete_publication),
        )
        .route(
            "/api/subscriptions/publications/{id}/sources",
            put(handlers::set_publication_sources),
        )
        .layer(axum::middleware::from_fn(trace::trace_request))
        .with_state(state)
}
