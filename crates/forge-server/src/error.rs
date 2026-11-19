//! The API error envelope (openapi.yaml `components.schemas.Error`):
//! every error response is `{"error": {"code", "message", "details"?}}`
//! with a stable machine-readable `code`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// An error that maps onto the API envelope.
#[derive(Debug)]
pub struct ApiError {
    /// HTTP status.
    pub status: StatusCode,
    /// Stable machine-readable code (e.g. `snippet_not_found`).
    pub code: String,
    /// Human-readable message (safe to show; never contains secrets
    /// or SQL details).
    pub message: String,
    /// Optional structured details (boxed: `serde_json::Value` is
    /// 72 bytes, and this error type is returned by every handler).
    pub details: Option<Box<serde_json::Value>>,
}

#[derive(Serialize)]
struct Envelope<'a> {
    error: Body<'a>,
}

#[derive(Serialize)]
struct Body<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl ApiError {
    /// Build an error.
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.to_string(),
            message: message.into(),
            details: None,
        }
    }

    /// 400 `validation_failed`.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "validation_failed", message)
    }

    /// 401 `unauthorized`.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", message)
    }

    /// 403 `forbidden`.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    /// 404 with a caller-chosen code.
    pub fn not_found(code: &str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    /// 409 with a caller-chosen code.
    pub fn conflict(code: &str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    /// 429 `rate_limited`.
    #[must_use]
    pub fn rate_limited() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many requests; slow down and retry later",
        )
    }

    /// 500 `internal_error`. The real cause is logged by the caller;
    /// the message is deliberately generic.
    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Internal server error",
        )
    }

    /// Attach structured details.
    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(Box::new(details));
        self
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.status.as_u16(),
            self.code,
            self.message
        )
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Envelope {
            error: Body {
                code: &self.code,
                message: &self.message,
                details: self.details.as_deref().cloned(),
            },
        };
        (self.status, axum::Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!(error = %err, "database error");
        Self::internal()
    }
}

impl From<crate::repo::RepoError> for ApiError {
    fn from(err: crate::repo::RepoError) -> Self {
        use crate::repo::RepoError;
        match err {
            RepoError::NotFound(what) => Self::not_found("not_found", format!("{what} not found")),
            RepoError::Conflict(msg) => Self::conflict("conflict", msg),
            RepoError::Sqlx(e) => e.into(),
        }
    }
}

/// Result alias for handlers.
pub type ApiResult<T> = Result<T, ApiError>;
