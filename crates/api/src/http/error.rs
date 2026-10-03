use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use contract::auth::{ErrorBody, ErrorResponse};

use auth::application::{AppError, PortError};
use rule_set::application::AppError as RuleSetError;
use subscription::application::AppError as SubscriptionError;

#[derive(Debug)]
pub enum ApiError {
    Validation(String),
    BadRequest(String),
    Conflict(&'static str, String),
    Unauthorized(String),
    Forbidden(String),
    NotFound,
    Internal(String),
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str, String) {
        match self {
            ApiError::Validation(message) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR",
                message.clone(),
            ),
            ApiError::BadRequest(message) => {
                (StatusCode::BAD_REQUEST, "BAD_REQUEST", message.clone())
            }
            ApiError::Conflict(code, message) => (StatusCode::CONFLICT, *code, message.clone()),
            ApiError::Unauthorized(message) => {
                (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message.clone())
            }
            ApiError::Forbidden(message) => (StatusCode::FORBIDDEN, "FORBIDDEN", message.clone()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND", "not found".to_string()),
            ApiError::Internal(message) => {
                log_internal(message);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "internal error".to_string(),
                )
            }
        }
    }
}

fn log_internal(message: &str) {
    tracing::error!(error_detail = message, "internal error");
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.parts();
        if status.is_client_error() {
            tracing::debug!(error_code = code, error_detail = %message, "request rejected");
        }
        (
            status,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: code.to_string(),
                    message,
                },
            }),
        )
            .into_response()
    }
}

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        match err {
            AppError::InvalidUsername | AppError::InvalidPassword => {
                ApiError::Validation(err.to_string())
            }
            AppError::UsernameTaken => ApiError::Conflict("USERNAME_TAKEN", err.to_string()),
            AppError::InvalidCredentials | AppError::InvalidToken => {
                ApiError::Unauthorized(err.to_string())
            }
            AppError::Passkey(message) => {
                tracing::warn!(error_detail = %message, "passkey ceremony failed");
                ApiError::Validation("passkey ceremony failed".to_string())
            }
            AppError::NotFound => ApiError::NotFound,
            AppError::Port(PortError::InvalidToken) => {
                ApiError::Unauthorized("invalid token".to_string())
            }
            AppError::Port(PortError::Failure(message)) => ApiError::Internal(message),
            AppError::Repository(error) => ApiError::Internal(error.to_string()),
            AppError::Credential(error) => ApiError::Internal(error.to_string()),
            AppError::Internal(message) => ApiError::Internal(message),
        }
    }
}

impl From<SubscriptionError> for ApiError {
    fn from(err: SubscriptionError) -> Self {
        match err {
            SubscriptionError::InvalidName
            | SubscriptionError::InvalidSourceUrl
            | SubscriptionError::InvalidSourceId
            | SubscriptionError::InvalidSecret
            | SubscriptionError::UnknownSource
            | SubscriptionError::DuplicateSource
            | SubscriptionError::TooManySources => ApiError::Validation(err.to_string()),
            SubscriptionError::SourceUrlTaken => {
                ApiError::Conflict("SOURCE_URL_TAKEN", err.to_string())
            }
            SubscriptionError::NotFound => ApiError::NotFound,
            SubscriptionError::Repository(error) => ApiError::Internal(error.to_string()),
            SubscriptionError::Port(error) => ApiError::Internal(error.to_string()),
            SubscriptionError::Internal(message) => ApiError::Internal(message),
        }
    }
}

impl From<RuleSetError> for ApiError {
    fn from(err: RuleSetError) -> Self {
        match err {
            RuleSetError::InvalidId
            | RuleSetError::InvalidName
            | RuleSetError::InvalidUrl
            | RuleSetError::InvalidPath
            | RuleSetError::InvalidInterval
            | RuleSetError::InvalidCategory
            | RuleSetError::InvalidContentFormat
            | RuleSetError::InvalidSourceKind
            | RuleSetError::MissingContent
            | RuleSetError::NotRemote
            | RuleSetError::ContentNotEditable => ApiError::Validation(err.to_string()),
            RuleSetError::NameTaken => {
                ApiError::Conflict("RULE_SET_NAME_TAKEN", err.to_string())
            }
            RuleSetError::NotFound => ApiError::NotFound,
            RuleSetError::Repository(error) => ApiError::Internal(error.to_string()),
            RuleSetError::Fetch(error) => ApiError::Internal(error.to_string()),
            RuleSetError::Internal(message) => ApiError::Internal(message),
        }
    }
}
