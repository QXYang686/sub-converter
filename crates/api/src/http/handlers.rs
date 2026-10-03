use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
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
use contract::subscription::{
    CreatePublicationRequest, CreateSourceRequest, PublicationResponse, RuleProviderResponse,
    SetPublicationSourcesRequest, SourceResponse, UpdatePublicationRequest, UpdateSourceRequest,
};
use contract::user::UserResponse;
use subscription::application::{
    CreatePublicationCommand, CreatePublicationHandler, CreateSourceCommand, CreateSourceHandler,
    DeletePublicationCommand, DeletePublicationHandler, DeleteSourceCommand, DeleteSourceHandler,
    GetPublicationCommand, GetPublicationContentCommand, GetPublicationContentHandler,
    GetPublicationHandler, GetSourceCommand, GetSourceContentCommand, GetSourceContentHandler,
    GetSourceHandler, GetSourceProviderContentCommand, GetSourceProviderContentHandler,
    ListPublicationsHandler, ListSourceProvidersCommand, ListSourceProvidersHandler,
    ListSourcesHandler, RefreshSourceHandler, ServePublicationCommand, ServePublicationHandler,
    SetPublicationSourcesCommand, SetPublicationSourcesHandler, SubscriptionFormat,
    UpdatePublicationCommand, UpdatePublicationHandler, UpdateSourceCommand, UpdateSourceHandler,
};
use subscription::{PublicationId, SourceId, SourceUrl, SubscriptionUserInfo};
use user::UserId;

use super::dto::{
    auth_response, creation_options, passkey_response, publication_response, refresh_auth_response,
    request_options, rule_provider_response, source_response, user_response,
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
    let user = handler.handle(token).await.map_err(ApiError::from)?;
    super::trace::record_user_id(&user.id);
    Ok(user)
}

fn parse_user_id(view: &UserView) -> Result<UserId, ApiError> {
    UserId::parse(&view.id).map_err(|_| ApiError::Unauthorized("invalid subject".to_string()))
}

fn spawn_source_refresh(state: &AppState, source_id: &str, url: &str) {
    let (Ok(source_id), Ok(url)) = (SourceId::parse(source_id), SourceUrl::new(url)) else {
        tracing::warn!("skipping source refresh for invalid source reference");
        return;
    };
    RefreshSourceHandler::new(
        state.fetcher.clone(),
        state.snapshots.clone(),
        state.sources.clone(),
        state.publications.clone(),
        state.publication_snapshots.clone(),
        state.subscription_clock.clone(),
        state.rule_providers.clone(),
    )
    .spawn(state.background.as_ref(), source_id, url);
}

async fn invalidate_source_publications(state: &AppState, source_id: &SourceId) {
    match state.publications.list_by_source_id(source_id).await {
        Ok(publications) => {
            for publication in publications {
                if let Err(err) = state.publication_snapshots.delete(publication.id()).await {
                    tracing::warn!(
                        publication_id = %publication.id(),
                        error = %err,
                        "failed to invalidate publication snapshot"
                    );
                }
            }
        }
        Err(err) => tracing::warn!(error = %err, "failed to list publications for invalidation"),
    }
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
    super::trace::record_user_id(&user.id);
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
    super::trace::record_user_id(&result.user.id);

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
            super::trace::record_user_id(&result.user.id);
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
    super::trace::record_user_id(&result.user.id);

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

pub async fn list_sources(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SourceResponse>>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = ListSourcesHandler::new(state.sources.clone(), state.snapshots.clone());
    let sources = handler.handle(&user_id).await?;
    Ok(Json(sources.into_iter().map(source_response).collect()))
}

pub async fn create_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: JsonPayload<CreateSourceRequest>,
) -> Result<(StatusCode, Json<SourceResponse>), ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;
    let Json(request) = payload.map_err(json_rejection)?;

    let handler = CreateSourceHandler::new(state.sources.clone(), state.subscription_clock.clone());
    let source = handler
        .handle(CreateSourceCommand {
            user_id,
            name: request.name,
            url: request.url,
            enabled: request.enabled,
        })
        .await?;
    if source.enabled {
        spawn_source_refresh(&state, &source.id, &source.url);
    }
    Ok((StatusCode::CREATED, Json(source_response(source))))
}

pub async fn get_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<SourceResponse>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = GetSourceHandler::new(state.sources.clone(), state.snapshots.clone());
    let source = handler
        .handle(GetSourceCommand {
            user_id,
            source_id: id,
        })
        .await?;
    Ok(Json(source_response(source)))
}

pub async fn get_source_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = GetSourceContentHandler::new(state.sources.clone(), state.snapshots.clone());
    let content = handler
        .handle(GetSourceContentCommand {
            user_id,
            source_id: id,
        })
        .await?;
    Ok(text_response(content))
}

pub async fn list_source_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<RuleProviderResponse>>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler =
        ListSourceProvidersHandler::new(state.sources.clone(), state.rule_providers.clone());
    let providers = handler
        .handle(ListSourceProvidersCommand {
            user_id,
            source_id: id,
        })
        .await?;
    Ok(Json(
        providers.into_iter().map(rule_provider_response).collect(),
    ))
}

pub async fn get_source_provider_content(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler =
        GetSourceProviderContentHandler::new(state.sources.clone(), state.rule_providers.clone());
    let content = handler
        .handle(GetSourceProviderContentCommand {
            user_id,
            source_id: id,
            name,
        })
        .await?;
    Ok(text_response(content))
}

pub async fn update_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: JsonPayload<UpdateSourceRequest>,
) -> Result<Json<SourceResponse>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;
    let Json(request) = payload.map_err(json_rejection)?;

    let refresh_target = SourceId::parse(&id).ok();
    let previous = match &refresh_target {
        Some(source_id) => state
            .sources
            .find_by_id(&user_id, source_id)
            .await
            .map_err(|err| ApiError::Internal(err.to_string()))?,
        None => None,
    };

    let handler = UpdateSourceHandler::new(state.sources.clone(), state.subscription_clock.clone());
    let source = handler
        .handle(UpdateSourceCommand {
            user_id,
            source_id: id,
            name: request.name,
            url: request.url,
            enabled: request.enabled,
        })
        .await?;

    let url_changed = previous
        .as_ref()
        .is_some_and(|old| old.url().value() != source.url);
    let reenabled = previous
        .as_ref()
        .is_some_and(|old| !old.enabled() && source.enabled);
    let enabled_changed = previous
        .as_ref()
        .is_some_and(|old| old.enabled() != source.enabled);
    if url_changed {
        if let Some(source_id) = &refresh_target {
            if let Err(err) = state.snapshots.clear(source_id).await {
                tracing::warn!(error = %err, "failed to clear source snapshot");
            }
        }
    }
    if let Some(source_id) = &refresh_target {
        if url_changed || enabled_changed {
            invalidate_source_publications(&state, source_id).await;
        }
    }
    if source.enabled && (url_changed || reenabled) {
        spawn_source_refresh(&state, &source.id, &source.url);
    }
    Ok(Json(source_response(source)))
}

pub async fn delete_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let source_id = SourceId::parse(&id).ok();
    if let Some(source_id) = &source_id {
        invalidate_source_publications(&state, source_id).await;
    }

    let handler = DeleteSourceHandler::new(state.sources.clone());
    handler
        .handle(DeleteSourceCommand {
            user_id,
            source_id: id,
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_publications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PublicationResponse>>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = ListPublicationsHandler::new(state.publications.clone());
    let publications = handler.handle(&user_id).await?;
    Ok(Json(
        publications.into_iter().map(publication_response).collect(),
    ))
}

pub async fn create_publication(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: JsonPayload<CreatePublicationRequest>,
) -> Result<(StatusCode, Json<PublicationResponse>), ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;
    let Json(request) = payload.map_err(json_rejection)?;

    let handler = CreatePublicationHandler::new(
        state.sources.clone(),
        state.publications.clone(),
        state.secret_generator.clone(),
        state.subscription_clock.clone(),
    );
    let publication = handler
        .handle(CreatePublicationCommand {
            user_id,
            name: request.name,
            source_ids: request.source_ids,
            expires_at: request.expires_at,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(publication_response(publication))))
}

pub async fn get_publication(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<PublicationResponse>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = GetPublicationHandler::new(state.publications.clone());
    let publication = handler
        .handle(GetPublicationCommand {
            user_id,
            publication_id: id,
        })
        .await?;
    Ok(Json(publication_response(publication)))
}

pub async fn update_publication(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: JsonPayload<UpdatePublicationRequest>,
) -> Result<Json<PublicationResponse>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;
    let Json(request) = payload.map_err(json_rejection)?;

    let handler =
        UpdatePublicationHandler::new(state.publications.clone(), state.subscription_clock.clone());
    let publication = handler
        .handle(UpdatePublicationCommand {
            user_id,
            publication_id: id,
            name: request.name,
            enabled: request.enabled,
            expires_at: request.expires_at,
        })
        .await?;
    Ok(Json(publication_response(publication)))
}

pub async fn get_publication_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = GetPublicationContentHandler::new(
        state.publications.clone(),
        state.sources.clone(),
        state.snapshots.clone(),
    );
    let content = handler
        .handle(GetPublicationContentCommand {
            user_id,
            publication_id: id,
        })
        .await?;
    Ok(text_response(content))
}

fn text_response(content: String) -> Response {
    let mut response = (StatusCode::OK, content).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/yaml; charset=utf-8"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub async fn delete_publication(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;

    let handler = DeletePublicationHandler::new(state.publications.clone());
    handler
        .handle(DeletePublicationCommand {
            user_id,
            publication_id: id,
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_publication_sources(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: JsonPayload<SetPublicationSourcesRequest>,
) -> Result<Json<PublicationResponse>, ApiError> {
    let user = current_user(&state, &headers).await?;
    let user_id = parse_user_id(&user)?;
    let Json(request) = payload.map_err(json_rejection)?;

    let handler = SetPublicationSourcesHandler::new(
        state.sources.clone(),
        state.publications.clone(),
        state.subscription_clock.clone(),
    );
    let publication = handler
        .handle(SetPublicationSourcesCommand {
            user_id,
            publication_id: id,
            source_ids: request.source_ids,
        })
        .await?;
    if let Ok(publication_id) = PublicationId::parse(&publication.id) {
        if let Err(err) = state.publication_snapshots.delete(&publication_id).await {
            tracing::warn!(
                publication_id = %publication.id,
                error = %err,
                "failed to invalidate publication snapshot"
            );
        }
    }
    Ok(Json(publication_response(publication)))
}

pub async fn public_subscription(
    State(state): State<AppState>,
    Path(secret): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let target = query.get("target").map(String::as_str).unwrap_or("clash");
    let format = SubscriptionFormat::parse(target)
        .ok_or_else(|| ApiError::BadRequest("unsupported target".to_string()))?;

    let handler = ServePublicationHandler::new(
        state.publications.clone(),
        state.sources.clone(),
        state.snapshots.clone(),
        state.publication_snapshots.clone(),
        Arc::new(RefreshSourceHandler::new(
            state.fetcher.clone(),
            state.snapshots.clone(),
            state.sources.clone(),
            state.publications.clone(),
            state.publication_snapshots.clone(),
            state.subscription_clock.clone(),
            state.rule_providers.clone(),
        )),
        state.background.clone(),
        state.subscription_clock.clone(),
    );
    let subscription = handler
        .handle(ServePublicationCommand { secret, format })
        .await?;

    let mut response = (StatusCode::OK, subscription.content).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/yaml; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&content_disposition(&subscription.name))
            .map_err(|_| ApiError::Internal("invalid content disposition".to_string()))?,
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        HeaderName::from_static("profile-update-interval"),
        HeaderValue::from_static("24"),
    );
    if let Some(userinfo) = subscription_userinfo_header(&subscription.userinfo) {
        if let Ok(value) = HeaderValue::from_str(&userinfo) {
            headers.insert(HeaderName::from_static("subscription-userinfo"), value);
        }
    }
    Ok(response)
}

fn subscription_userinfo_header(info: &SubscriptionUserInfo) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(upload) = info.upload {
        parts.push(format!("upload={upload}"));
    }
    if let Some(download) = info.download {
        parts.push(format!("download={download}"));
    }
    if let Some(total) = info.total {
        parts.push(format!("total={total}"));
    }
    if let Some(expire) = info.expire {
        parts.push(format!("expire={expire}"));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

fn content_disposition(name: &str) -> String {
    let fallback: String = name
        .chars()
        .map(|character| match character {
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' => c,
            _ => '_',
        })
        .collect();
    let encoded: String = name
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect();
    format!("attachment; filename=\"{fallback}.yaml\"; filename*=UTF-8''{encoded}.yaml")
}

#[cfg(test)]
mod tests {
    use super::{content_disposition, subscription_userinfo_header};
    use subscription::SubscriptionUserInfo;

    #[test]
    fn content_disposition_encodes_unicode_names() {
        let value = content_disposition("我的订阅");
        assert!(value.contains("filename=\"____.yaml\""), "{value}");
        assert!(
            value.contains("filename*=UTF-8''%E6%88%91%E7%9A%84%E8%AE%A2%E9%98%85.yaml"),
            "{value}"
        );
    }

    #[test]
    fn content_disposition_keeps_ascii_names_readable() {
        let value = content_disposition("my-sub");
        assert!(value.contains("filename=\"my-sub.yaml\""), "{value}");
    }

    #[test]
    fn subscription_userinfo_header_formats_present_fields() {
        let info = SubscriptionUserInfo {
            upload: Some(1),
            download: Some(2),
            total: Some(3),
            expire: None,
        };
        assert_eq!(
            subscription_userinfo_header(&info).as_deref(),
            Some("upload=1; download=2; total=3")
        );
        assert_eq!(
            subscription_userinfo_header(&SubscriptionUserInfo::default()),
            None
        );
    }
}
