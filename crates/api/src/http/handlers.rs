use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use contract::auth::{LoginRequest, RegisterRequest};
use contract::user::UserResponse;

use user::application::config::REFRESH_TOKEN_TTL_SECONDS;
use user::application::{
    AppError, GetCurrentUserHandler, LoginCommand, LoginHandler, LogoutCommand, LogoutHandler,
    RefreshCommand, RefreshHandler, RegisterCommand, RegisterHandler,
};

use super::dto::{auth_response, refresh_auth_response, user_response};
use super::error::ApiError;
use super::session;
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
    headers: HeaderMap,
    payload: JsonPayload<LoginRequest>,
) -> Result<Response, ApiError> {
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

    let cookie = session::set_refresh_cookie(
        &result.refresh_token,
        REFRESH_TOKEN_TTL_SECONDS,
        session::is_secure_request(&headers),
    )?;
    let mut response = Json(auth_response(result)).into_response();
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(response)
}

pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    session::require_xhr(&headers)?;
    let secure = session::is_secure_request(&headers);
    let refresh_token = session::refresh_token_from_headers(&headers)
        .ok_or_else(|| ApiError::Unauthorized("missing refresh token".to_string()))?;

    let handler = RefreshHandler::new(
        state.users.clone(),
        state.token_service.clone(),
        state.refresh_tokens.clone(),
        state.clock.clone(),
    );
    match handler
        .handle(RefreshCommand { refresh_token })
        .await
    {
        Ok(result) => {
            let cookie =
                session::set_refresh_cookie(&result.refresh_token, REFRESH_TOKEN_TTL_SECONDS, secure)?;
            let mut response = Json(refresh_auth_response(result)).into_response();
            response.headers_mut().append(header::SET_COOKIE, cookie);
            Ok(response)
        }
        Err(AppError::InvalidToken) => {
            let mut response = ApiError::Unauthorized("invalid token".to_string()).into_response();
            response
                .headers_mut()
                .append(header::SET_COOKIE, session::clear_refresh_cookie(secure));
            Ok(response)
        }
        Err(err) => Err(err.into()),
    }
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    session::require_xhr(&headers)?;
    let secure = session::is_secure_request(&headers);

    if let Some(refresh_token) = session::refresh_token_from_headers(&headers) {
        let handler = LogoutHandler::new(
            state.token_service.clone(),
            state.refresh_tokens.clone(),
            state.clock.clone(),
        );
        handler.handle(LogoutCommand { refresh_token }).await?;
    }

    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(header::SET_COOKIE, session::clear_refresh_cookie(secure));
    Ok(response)
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
