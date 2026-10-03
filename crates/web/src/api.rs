mod client;

pub use client::{user_message, ApiError};

use serde::de::DeserializeOwned;

use contract::auth::{AuthResponse, LoginRequest, RegisterRequest};
use contract::passkey::{
    PasskeyLoginFinishRequest, PasskeyLoginStartRequest, PasskeyResponse,
    PublicKeyCredentialCreationOptions, PublicKeyCredentialRequestOptions,
    RegisterPasskeyFinishRequest,
};
use contract::rule_set::{
    CreateRuleSetRequest, RefreshRuleSetResponse, RuleSetResponse, SetRuleSetPinnedRequest,
    UpdateRuleSetRequest,
};
use contract::subscription::{
    CreatePublicationRequest, CreateSourceRequest, PublicationResponse, RuleProviderResponse,
    SetPublicationSourcesRequest, SourceResponse, UpdatePublicationRequest, UpdateSourceRequest,
};
use contract::user::UserResponse;

use self::client::{delete, get, patch_json, post, post_json, put_json, put_text};

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

pub async fn passkey_register_start(
    access_token: &str,
) -> Result<PublicKeyCredentialCreationOptions, ApiError> {
    let text = post("/api/auth/passkey/register/start", Some(access_token)).await?;
    parse(&text)
}

pub async fn passkey_register_finish(
    access_token: &str,
    request: &RegisterPasskeyFinishRequest,
) -> Result<PasskeyResponse, ApiError> {
    let text = post_json(
        "/api/auth/passkey/register/finish",
        request,
        Some(access_token),
    )
    .await?;
    parse(&text)
}

pub async fn passkey_login_start(
    username: Option<&str>,
) -> Result<PublicKeyCredentialRequestOptions, ApiError> {
    let body = PasskeyLoginStartRequest {
        username: username.map(str::to_string),
    };
    let text = post_json("/api/auth/passkey/login/start", &body, None).await?;
    parse(&text)
}

pub async fn passkey_login_finish(
    request: &PasskeyLoginFinishRequest,
) -> Result<AuthResponse, ApiError> {
    let text = post_json("/api/auth/passkey/login/finish", request, None).await?;
    parse(&text)
}

pub async fn list_passkeys(access_token: &str) -> Result<Vec<PasskeyResponse>, ApiError> {
    let text = get("/api/auth/passkeys", Some(access_token)).await?;
    parse(&text)
}

pub async fn delete_passkey(access_token: &str, id: &str) -> Result<(), ApiError> {
    delete(&format!("/api/auth/passkeys/{id}"), Some(access_token))
        .await
        .map(|_| ())
}

pub async fn list_sources(access_token: &str) -> Result<Vec<SourceResponse>, ApiError> {
    let text = get("/api/subscriptions/sources", Some(access_token)).await?;
    parse(&text)
}

pub async fn create_source(
    access_token: &str,
    request: &CreateSourceRequest,
) -> Result<SourceResponse, ApiError> {
    let text = post_json("/api/subscriptions/sources", request, Some(access_token)).await?;
    parse(&text)
}

pub async fn update_source(
    access_token: &str,
    id: &str,
    request: &UpdateSourceRequest,
) -> Result<SourceResponse, ApiError> {
    let text = patch_json(
        &format!("/api/subscriptions/sources/{id}"),
        request,
        Some(access_token),
    )
    .await?;
    parse(&text)
}

pub async fn delete_source(access_token: &str, id: &str) -> Result<(), ApiError> {
    delete(
        &format!("/api/subscriptions/sources/{id}"),
        Some(access_token),
    )
    .await
    .map(|_| ())
}

pub async fn source_config(access_token: &str, id: &str) -> Result<String, ApiError> {
    get(
        &format!("/api/subscriptions/sources/{id}/config"),
        Some(access_token),
    )
    .await
}

pub async fn source_providers(
    access_token: &str,
    id: &str,
) -> Result<Vec<RuleProviderResponse>, ApiError> {
    let text = get(
        &format!("/api/subscriptions/sources/{id}/providers"),
        Some(access_token),
    )
    .await?;
    parse(&text)
}

pub async fn source_provider_content(
    access_token: &str,
    id: &str,
    name: &str,
) -> Result<String, ApiError> {
    get(
        &format!("/api/subscriptions/sources/{id}/providers/{name}"),
        Some(access_token),
    )
    .await
}

pub async fn list_publications(access_token: &str) -> Result<Vec<PublicationResponse>, ApiError> {
    let text = get("/api/subscriptions/publications", Some(access_token)).await?;
    parse(&text)
}

pub async fn create_publication(
    access_token: &str,
    request: &CreatePublicationRequest,
) -> Result<PublicationResponse, ApiError> {
    let text = post_json(
        "/api/subscriptions/publications",
        request,
        Some(access_token),
    )
    .await?;
    parse(&text)
}

pub async fn update_publication(
    access_token: &str,
    id: &str,
    request: &UpdatePublicationRequest,
) -> Result<PublicationResponse, ApiError> {
    let text = patch_json(
        &format!("/api/subscriptions/publications/{id}"),
        request,
        Some(access_token),
    )
    .await?;
    parse(&text)
}

pub async fn set_publication_sources(
    access_token: &str,
    id: &str,
    request: &SetPublicationSourcesRequest,
) -> Result<PublicationResponse, ApiError> {
    let text = put_json(
        &format!("/api/subscriptions/publications/{id}/sources"),
        request,
        Some(access_token),
    )
    .await?;
    parse(&text)
}

pub async fn publication_config(access_token: &str, id: &str) -> Result<String, ApiError> {
    get(
        &format!("/api/subscriptions/publications/{id}/config"),
        Some(access_token),
    )
    .await
}

pub async fn delete_publication(access_token: &str, id: &str) -> Result<(), ApiError> {
    delete(
        &format!("/api/subscriptions/publications/{id}"),
        Some(access_token),
    )
    .await
    .map(|_| ())
}

pub async fn list_rule_sets(access_token: &str) -> Result<Vec<RuleSetResponse>, ApiError> {
    let text = get("/api/rule-sets", Some(access_token)).await?;
    parse(&text)
}

pub async fn create_rule_set(
    access_token: &str,
    request: &CreateRuleSetRequest,
) -> Result<RuleSetResponse, ApiError> {
    let text = post_json("/api/rule-sets", request, Some(access_token)).await?;
    parse(&text)
}

pub async fn update_rule_set(
    access_token: &str,
    id: &str,
    request: &UpdateRuleSetRequest,
) -> Result<RuleSetResponse, ApiError> {
    let text = patch_json(&format!("/api/rule-sets/{id}"), request, Some(access_token)).await?;
    parse(&text)
}

pub async fn delete_rule_set(access_token: &str, id: &str) -> Result<(), ApiError> {
    delete(&format!("/api/rule-sets/{id}"), Some(access_token))
        .await
        .map(|_| ())
}

pub async fn refresh_rule_set(
    access_token: &str,
    id: &str,
) -> Result<RefreshRuleSetResponse, ApiError> {
    let text = post(&format!("/api/rule-sets/{id}/refresh"), Some(access_token)).await?;
    parse(&text)
}

pub async fn rule_set_content(access_token: &str, id: &str) -> Result<String, ApiError> {
    get(&format!("/api/rule-sets/{id}/content"), Some(access_token)).await
}

pub async fn replace_rule_set_content(
    access_token: &str,
    id: &str,
    content: &str,
) -> Result<RuleSetResponse, ApiError> {
    let text = put_text(&format!("/api/rule-sets/{id}/content"), content, Some(access_token)).await?;
    parse(&text)
}

pub async fn set_rule_set_pinned(
    access_token: &str,
    id: &str,
    pinned: bool,
) -> Result<RuleSetResponse, ApiError> {
    let request = SetRuleSetPinnedRequest { pinned };
    let text = patch_json(
        &format!("/api/rule-sets/{id}/content"),
        &request,
        Some(access_token),
    )
    .await?;
    parse(&text)
}

fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ApiError> {
    serde_json::from_str(text).map_err(|err| ApiError::invalid_response(err.to_string()))
}
