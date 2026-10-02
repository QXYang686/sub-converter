use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;

use user::application::{
    GetCurrentUserHandler, LoginCommand, LoginHandler, LogoutCommand, LogoutHandler,
    RefreshCommand, RefreshHandler, RegisterCommand, RegisterHandler,
};

use contract::auth::{
    AuthResponse, LoginRequest, LogoutRequest, RefreshRequest, RegisterRequest,
};
use contract::user::UserResponse;

use super::dto::{auth_response, refresh_auth_response, user_response};
use super::error::ApiError;
use super::state::AppState;

type JsonPayload<T> = Result<Json<T>, JsonRejection>;

fn json_rejection(rejection: JsonRejection) -> ApiError {
    ApiError::Validation(rejection.body_text())
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

pub async fn register(
    State(state): State<AppState>,
    payload: JsonPayload<RegisterRequest>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    let Json(request) = payload.map_err(json_rejection)?;
    let handler = RegisterHandler::new(
        state.users.clone(),
        state.password_hasher.clone(),
        state.clock.clone(),
    );
    let user = handler
        .handle(RegisterCommand {
            username: request.username,
            password: request.password,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(user_response(user))))
}

pub async fn login(
    State(state): State<AppState>,
    payload: JsonPayload<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let Json(request) = payload.map_err(json_rejection)?;
    let handler = LoginHandler::new(
        state.users.clone(),
        state.password_hasher.clone(),
        state.token_service.clone(),
        state.refresh_tokens.clone(),
        state.clock.clone(),
    );
    let result = handler
        .handle(LoginCommand {
            username: request.username,
            password: request.password,
        })
        .await?;
    Ok(Json(auth_response(result)))
}

pub async fn refresh(
    State(state): State<AppState>,
    payload: JsonPayload<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let Json(request) = payload.map_err(json_rejection)?;
    let handler = RefreshHandler::new(
        state.users.clone(),
        state.token_service.clone(),
        state.refresh_tokens.clone(),
        state.clock.clone(),
    );
    let result = handler
        .handle(RefreshCommand {
            refresh_token: request.refresh_token,
        })
        .await?;
    Ok(Json(refresh_auth_response(result)))
}

pub async fn logout(
    State(state): State<AppState>,
    payload: JsonPayload<LogoutRequest>,
) -> Result<StatusCode, ApiError> {
    let Json(request) = payload.map_err(json_rejection)?;
    let handler = LogoutHandler::new(
        state.token_service.clone(),
        state.refresh_tokens.clone(),
        state.clock.clone(),
    );
    handler
        .handle(LogoutCommand {
            refresh_token: request.refresh_token,
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserResponse>, ApiError> {
    let token = bearer_token(&headers)
        .ok_or_else(|| ApiError::Unauthorized("missing bearer token".to_string()))?;
    let handler = GetCurrentUserHandler::new(
        state.users.clone(),
        state.token_service.clone(),
        state.clock.clone(),
    );
    let user = handler.handle(token).await?;
    Ok(Json(user_response(user)))
}
