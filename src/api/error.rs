use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Auth error: {0}")]
    AuthError(#[from] crate::auth::types::AuthError),

    #[error("Storage error: {0}")]
    StorageError(#[from] crate::storage::error::StorageError),

    #[error("Internal server error")]
    InternalServerError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            ApiError::KeyNotFound(_) => StatusCode::NOT_FOUND,
            ApiError::PermissionDenied(_) => StatusCode::FORBIDDEN,
            ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::AuthError(_) => StatusCode::UNAUTHORIZED,
            ApiError::StorageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = serde_json::json!({
            "error": self.to_string(),
        });

        (status, axum::Json(body)).into_response()
    }
}