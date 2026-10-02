use contract::auth::AuthResponse;
use contract::user::UserResponse;
use user::application::{LoginResult, RefreshResult, UserView};

pub fn user_response(view: UserView) -> UserResponse {
    UserResponse {
        id: view.id,
        username: view.username,
    }
}

pub fn auth_response(result: LoginResult) -> AuthResponse {
    AuthResponse {
        access_token: result.access_token,
        access_token_expires_at: result.access_token_expires_at,
        refresh_token: result.refresh_token,
        refresh_token_expires_at: result.refresh_token_expires_at,
        user: user_response(result.user),
    }
}

pub fn refresh_auth_response(result: RefreshResult) -> AuthResponse {
    AuthResponse {
        access_token: result.access_token,
        access_token_expires_at: result.access_token_expires_at,
        refresh_token: result.refresh_token,
        refresh_token_expires_at: result.refresh_token_expires_at,
        user: user_response(result.user),
    }
}
