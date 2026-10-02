use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use contract::auth::{ErrorBody, ErrorResponse};

use user::application::{AppError, PortError};
use user::domain::DomainError;

#[derive(Debug)]
pub enum ApiError {
    Validation(String),
    Conflict(String),
    Unauthorized(String),
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
            ApiError::Conflict(message) => {
                (StatusCode::CONFLICT, "USERNAME_TAKEN", message.clone())
            }
            ApiError::Unauthorized(message) => {
                (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message.clone())
            }
            ApiError::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                message.clone(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.parts();
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
            AppError::Domain(error) => {
                let validation = error == DomainError::InvalidUsername
                    || error == DomainError::InvalidPassword;
                if validation {
                    ApiError::Validation(error.to_string())
                } else {
                    ApiError::Internal(error.to_string())
                }
            }
            AppError::Repository(error) => ApiError::Internal(error.to_string()),
            AppError::Port(error) => match error {
                PortError::InvalidToken => ApiError::Unauthorized(error.to_string()),
                PortError::Failure(message) => ApiError::Internal(message),
            },
            AppError::UsernameTaken => ApiError::Conflict(err.to_string()),
            AppError::InvalidCredentials => ApiError::Unauthorized(err.to_string()),
            AppError::InvalidToken => ApiError::Unauthorized(err.to_string()),
            AppError::Internal(message) => ApiError::Internal(message),
        }
    }
}
