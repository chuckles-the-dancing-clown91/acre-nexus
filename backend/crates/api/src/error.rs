//! Unified API error type that serialises to a consistent JSON envelope and maps
//! to appropriate HTTP status codes.

use rocket::http::{ContentType, Status};
use rocket::response::{self, Responder};
use rocket::serde::json::serde_json;
use rocket::Request;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    // Part of the error surface used by upcoming write endpoints (unique-constraint hits).
    #[allow(dead_code)]
    #[error("{0}")]
    Conflict(String),
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
    #[error("database error")]
    Db(#[from] sea_orm::DbErr),
}

impl ApiError {
    fn status(&self) -> Status {
        match self {
            ApiError::BadRequest(_) => Status::BadRequest,
            ApiError::Unauthorized => Status::Unauthorized,
            ApiError::Forbidden(_) => Status::Forbidden,
            ApiError::NotFound(_) => Status::NotFound,
            ApiError::Conflict(_) => Status::Conflict,
            ApiError::Internal(_) | ApiError::Db(_) => Status::InternalServerError,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Unauthorized => "unauthorized",
            ApiError::Forbidden(_) => "forbidden",
            ApiError::NotFound(_) => "not_found",
            ApiError::Conflict(_) => "conflict",
            ApiError::Internal(_) | ApiError::Db(_) => "internal",
        }
    }
}

impl<'r> Responder<'r, 'static> for ApiError {
    fn respond_to(self, _req: &'r Request<'_>) -> response::Result<'static> {
        let status = self.status();
        // Don't leak internal/database detail to clients.
        let message = match &self {
            ApiError::Internal(_) | ApiError::Db(_) => "Internal server error".to_string(),
            other => other.to_string(),
        };
        if let ApiError::Db(e) = &self {
            tracing::error!("db error: {e}");
        }
        if let ApiError::Internal(e) = &self {
            tracing::error!("internal error: {e:?}");
        }
        let body = serde_json::json!({
            "error": { "code": self.code(), "message": message }
        })
        .to_string();
        response::Response::build()
            .status(status)
            .header(ContentType::JSON)
            .sized_body(body.len(), std::io::Cursor::new(body))
            .ok()
    }
}

/// Convenient result alias for handlers.
pub type ApiResult<T> = Result<T, ApiError>;
