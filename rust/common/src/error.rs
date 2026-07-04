//! Unified error type rendered as RFC 7807 `application/problem+json`.
//!
//! Handlers return `Result<T, AppError>`; `AppError` knows its HTTP status and
//! serializes a consistent problem document. Internal causes are logged but
//! never leaked to clients.

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),

    #[error("authentication required")]
    Unauthorized,

    #[error("insufficient permissions")]
    Forbidden,

    #[error("{0} not found")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    #[error("rate limit exceeded")]
    TooManyRequests,

    #[error("request validation failed")]
    Validation(#[from] validator::ValidationErrors),

    #[error("upstream service unavailable: {0}")]
    Upstream(String),

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn not_found(resource: impl Into<String>) -> Self {
        AppError::NotFound(resource.into())
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        AppError::BadRequest(msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        AppError::Conflict(msg.into())
    }

    fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) | AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            AppError::Upstream(_) => StatusCode::BAD_GATEWAY,
            AppError::Database(e) => Self::map_db_status(e),
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Translate well-known database errors into client-meaningful statuses.
    fn map_db_status(e: &sqlx::Error) -> StatusCode {
        match e {
            sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
            sqlx::Error::Database(db) => match db.code().as_deref() {
                Some("23505") => StatusCode::CONFLICT,    // unique_violation
                Some("23503") => StatusCode::CONFLICT,    // foreign_key_violation
                Some("23514") => StatusCode::BAD_REQUEST, // check_violation
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn problem_type(&self) -> &'static str {
        match self {
            AppError::BadRequest(_) => "https://errors.ecommerce.dev/bad-request",
            AppError::Validation(_) => "https://errors.ecommerce.dev/validation",
            AppError::Unauthorized => "https://errors.ecommerce.dev/unauthorized",
            AppError::Forbidden => "https://errors.ecommerce.dev/forbidden",
            AppError::NotFound(_) => "https://errors.ecommerce.dev/not-found",
            AppError::Conflict(_) => "https://errors.ecommerce.dev/conflict",
            AppError::TooManyRequests => "https://errors.ecommerce.dev/rate-limit",
            AppError::Upstream(_) => "https://errors.ecommerce.dev/upstream",
            AppError::Database(_) | AppError::Internal(_) => {
                "https://errors.ecommerce.dev/internal"
            }
        }
    }
}

/// RFC 7807 problem document.
#[derive(Debug, Serialize)]
struct ProblemDetails {
    #[serde(rename = "type")]
    type_: String,
    title: String,
    status: u16,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();

        // Server-side faults are logged with full context; clients get a generic message.
        let detail = match &self {
            AppError::Database(e) if status == StatusCode::INTERNAL_SERVER_ERROR => {
                tracing::error!(error = %e, "database error");
                "an internal error occurred".to_string()
            }
            AppError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                "an internal error occurred".to_string()
            }
            other => other.to_string(),
        };

        let errors = match &self {
            AppError::Validation(v) => serde_json::to_value(v).ok(),
            _ => None,
        };

        let body = ProblemDetails {
            type_: self.problem_type().to_string(),
            title: status.canonical_reason().unwrap_or("Error").to_string(),
            status: status.as_u16(),
            detail,
            errors,
        };

        (
            status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            axum::Json(body),
        )
            .into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
