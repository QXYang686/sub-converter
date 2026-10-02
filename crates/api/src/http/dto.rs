use serde::{Deserialize, Serialize};

use user::application::{LoginResult, RefreshResult, UserView};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: String,
    pub username: String,
}

impl From<UserView> for UserResponse {
    fn from(view: UserView) -> Self {
        Self {
            id: view.id,
            username: view.username,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub access_token: String,
    pub access_token_expires_at: i64,
    pub refresh_token: String,
    pub refresh_token_expires_at: i64,
    pub user: UserResponse,
}

impl From<LoginResult> for AuthResponse {
    fn from(result: LoginResult) -> Self {
        Self {
            access_token: result.access_token,
            access_token_expires_at: result.access_token_expires_at,
            refresh_token: result.refresh_token,
            refresh_token_expires_at: result.refresh_token_expires_at,
            user: result.user.into(),
        }
    }
}

impl From<RefreshResult> for AuthResponse {
    fn from(result: RefreshResult) -> Self {
        Self {
            access_token: result.access_token,
            access_token_expires_at: result.access_token_expires_at,
            refresh_token: result.refresh_token,
            refresh_token_expires_at: result.refresh_token_expires_at,
            user: result.user.into(),
        }
    }
}
