use axum::extract::{Path, Query, State};
use axum::Json;
use base64::Engine;
use std::sync::Arc;

use crate::api::auth_middleware::AuthenticatedUser;
use crate::api::error::ApiError;
use crate::api::rest::types::*;
use crate::storage::StorageEngine;

pub async fn get_handler(
    State(engine): State<Arc<StorageEngine>>,
    AuthenticatedUser(auth_ctx): AuthenticatedUser,
    Query(params): Query<GetParams>,
) -> Result<Json<GetResponse>, ApiError> {
    engine
        .get(&params.key)
        .await
        .map_err(ApiError::StorageError)?;

    // Authorize
    auth_ctx
        .authorize(&auth_ctx, "GET", &params.key)
        .map_err(ApiError::AuthError)?;

    let entry = engine.get(&params.key).await?;
    let value_b64 = base64::engine::general_purpose::STANDARD.encode(&entry.value);

    Ok(Json(GetResponse {
        found: true,
        value: Some(value_b64),
        version: entry.version,
    }))
}

pub async fn set_handler(
    State(engine): State<Arc<StorageEngine>>,
    AuthenticatedUser(auth_ctx): AuthenticatedUser,
    Json(params): Json<SetParams>,
) -> Result<Json<SetResponse>, ApiError> {
    auth_ctx
        .authorize(&auth_ctx, "SET", &params.key)
        .map_err(ApiError::AuthError)?;

    let value = base64::engine::general_purpose::STANDARD
        .decode(&params.value)
        .map_err(|_| ApiError::InvalidRequest("Invalid base64 value".to_string()))?;

    engine.set(&params.key, value, params.ttl).await?;

    // For now, version is always 1
    Ok(Json(SetResponse {
        success: true,
        version: 1,
    }))
}

pub async fn delete_handler(
    State(engine): State<Arc<StorageEngine>>,
    AuthenticatedUser(auth_ctx): AuthenticatedUser,
    Json(params): Json<DeleteParams>,
) -> Result<Json<DeleteResponse>, ApiError> {
    auth_ctx
        .authorize(&auth_ctx, "DEL", &params.key)
        .map_err(ApiError::AuthError)?;

    engine.del(&params.key, None).await?;

    Ok(Json(DeleteResponse { success: true }))
}