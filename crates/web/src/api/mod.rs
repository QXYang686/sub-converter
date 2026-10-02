mod client;

pub use client::{user_message, ApiError};

use serde::de::DeserializeOwned;

use contract::auth::{AuthResponse, LoginRequest, RegisterRequest};
use contract::user::UserResponse;

use self::client::{get, post, post_json};

pub async fn register(username: &str, password: &str) -> Result<UserResponse, ApiError> {
    let body = RegisterRequest {
        username: username.to_string(),
        password: password.to_string(),
    };
    let text = post_json("/api/auth/register", &body, None).await?;
    parse(&text)
}

pub async fn login(username: &str, password: &str) -> Result<AuthResponse, ApiError> {
    let body = LoginRequest {
        username: username.to_string(),
        password: password.to_string(),
    };
    let text = post_json("/api/auth/login", &body, None).await?;
    parse(&text)
}

pub async fn refresh() -> Result<AuthResponse, ApiError> {
    let text = post("/api/auth/refresh", None).await?;
    parse(&text)
}

pub async fn logout() -> Result<(), ApiError> {
    post("/api/auth/logout", None).await.map(|_| ())
}

pub async fn current_user(access_token: &str) -> Result<UserResponse, ApiError> {
    let text = get("/api/users/me", Some(access_token)).await?;
    parse(&text)
}

fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ApiError> {
    serde_json::from_str(text).map_err(|err| ApiError::invalid_response(err.to_string()))
}
