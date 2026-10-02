use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use auth::application::config::REFRESH_TOKEN_TTL_SECONDS;
use auth::application::{
    AppError, DeletePasskeyCommand, DeletePasskeyHandler, FinishPasskeyLoginCommand,
    FinishPasskeyLoginHandler, FinishPasskeyRegistrationCommand, FinishPasskeyRegistrationHandler,
    GetCurrentUserHandler, ListPasskeysHandler, LoginCommand, LoginHandler, LogoutCommand,
    LogoutHandler, RefreshCommand, RefreshHandler, RegisterCommand, RegisterHandler,
    StartPasskeyLoginCommand, StartPasskeyLoginHandler, StartPasskeyRegistrationCommand,
    StartPasskeyRegistrationHandler, UserView,
};
use contract::auth::{LoginRequest, RegisterRequest};
use contract::passkey::{
    PasskeyLoginFinishRequest, PasskeyLoginStartRequest, PasskeyResponse,
    PublicKeyCredentialCreationOptions, PublicKeyCredentialRequestOptions,
    RegisterPasskeyFinishRequest,
};
use contract::user::UserResponse;
use user::UserId;

use super::dto::{
    auth_response, creation_options, passkey_response, refresh_auth_response, request_options,
    user_response,
};
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

async fn current_user(state: &AppState, headers: &HeaderMap) -> Result<UserView, ApiError> {
    let token = bearer_token(headers)
        .ok_or_else(|| ApiError::Unauthorized("missing bearer token".to_string()))?;
    let handler = GetCurrentUserHandler::new(
        state.users.clone(),
        state.token_service.clone(),
        state.clock.clone(),
    );
    handler.handle(token).await.map_err(Into::into)
}

fn parse_user_id(view: &UserView) -> Result<UserId, ApiError> {
    UserId::parse(&view.id).map_err(|_| ApiError::Unauthorized("invalid subject".to_string()))
}

pub async fn register(
    State(state): State<AppState>,
    payload: JsonPayload<RegisterRequest>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    let Json(request) = payload.map_err(json_rejection)?;
    let handler = RegisterHandler::new(
        state.users.clone(),
        state.credentials.clone(),
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
        state.credentials.clone(),
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
    match handler.handle(RefreshCommand { refresh_token }).await {
        Ok(result) => {
            let cookie = session::set_refresh_cookie(
                &result.refresh_token,
                REFRESH_TOKEN_TTL_SECONDS,
                secure,
            )?;
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
    let user = current_user(&state, &headers).await?;
    Ok(Json(user_response(user)))
}

pub async fn passkey_register_start(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PublicKeyCredentialCreationOptions>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = StartPasskeyRegistrationHandler::new(
        state.credentials.clone(),
        state.challenges.clone(),
        state.random.clone(),
        state.webauthn.clone(),
        state.clock.clone(),
    );
    let options = handler
        .handle(StartPasskeyRegistrationCommand {
            user_id,
            username: user.username,
        })
        .await?;
    Ok(Json(creation_options(options)))
}

pub async fn passkey_register_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: JsonPayload<RegisterPasskeyFinishRequest>,
) -> Result<Json<PasskeyResponse>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;
    let Json(request) = payload.map_err(json_rejection)?;

    let handler = FinishPasskeyRegistrationHandler::new(
        state.credentials.clone(),
        state.challenges.clone(),
        state.webauthn.clone(),
        state.clock.clone(),
    );
    let registered = handler
        .handle(FinishPasskeyRegistrationCommand {
            user_id,
            credential_id: request.credential_id,
            client_data_json: request.client_data_json,
            attestation_object: request.attestation_object,
            transports: request.transports,
            label: request.label,
        })
        .await?;

    Ok(Json(PasskeyResponse {
        id: registered.id,
        label: registered.label,
        transports: registered.transports,
        created_at: registered.created_at,
        last_used_at: None,
    }))
}

pub async fn passkey_login_start(
    State(state): State<AppState>,
    payload: JsonPayload<PasskeyLoginStartRequest>,
) -> Result<Json<PublicKeyCredentialRequestOptions>, ApiError> {
    let Json(request) = payload.map_err(json_rejection)?;
    let handler = StartPasskeyLoginHandler::new(
        state.users.clone(),
        state.credentials.clone(),
        state.challenges.clone(),
        state.random.clone(),
        state.webauthn.clone(),
        state.clock.clone(),
    );
    let options = handler
        .handle(StartPasskeyLoginCommand {
            username: request.username,
        })
        .await?;
    Ok(Json(request_options(options)))
}

pub async fn passkey_login_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: JsonPayload<PasskeyLoginFinishRequest>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(json_rejection)?;
    let handler = FinishPasskeyLoginHandler::new(
        state.users.clone(),
        state.credentials.clone(),
        state.challenges.clone(),
        state.token_service.clone(),
        state.refresh_tokens.clone(),
        state.webauthn.clone(),
        state.clock.clone(),
    );
    let result = handler
        .handle(FinishPasskeyLoginCommand {
            credential_id: request.credential_id,
            client_data_json: request.client_data_json,
            authenticator_data: request.authenticator_data,
            signature: request.signature,
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

pub async fn list_passkeys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PasskeyResponse>>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = ListPasskeysHandler::new(state.credentials.clone());
    let passkeys = handler.handle(&user_id).await?;
    Ok(Json(passkeys.into_iter().map(passkey_response).collect()))
}

pub async fn delete_passkey(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(credential_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = DeletePasskeyHandler::new(state.credentials.clone());
    handler
        .handle(DeletePasskeyCommand {
            user_id,
            credential_id,
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
