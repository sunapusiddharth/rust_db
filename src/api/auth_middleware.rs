use axum::async_trait;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::RequestPartsExt;
use std::net::SocketAddr;

use crate::auth::AuthManager;
use crate::auth::types::AuthContext;

#[derive(Clone)]
pub struct AuthState {
    pub auth_manager: std::sync::Arc<AuthManager>,
}

#[derive(Debug)]
pub struct AuthenticatedUser(pub AuthContext);

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = crate::api::error::ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let State(auth_state) = parts
            .extract_with_state::<State<AuthState>, _>(state)
            .await
            .map_err(|_| crate::api::error::ApiError::AuthError(crate::auth::types::AuthError::InvalidCredentials))?;

        let headers = &parts.headers;
        let source_ip = parts
            .extensions
            .get::<axum::extract::ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
            .unwrap_or("127.0.0.1".parse().unwrap());

        // Check API Key
        if let Some(api_key) = headers.get("X-API-Key") {
            if let Ok(key_str) = api_key.to_str() {
                let ctx = auth_state
                    .auth_manager
                    .authenticate_api_key(key_str, source_ip)
                    .await
                    .map_err(crate::api::error::ApiError::AuthError)?;
                return Ok(AuthenticatedUser(ctx));
            }
        }

        // Check JWT
        if let Some(auth_header) = headers.get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    let ctx = auth_state
                        .auth_manager
                        .authenticate_jwt(token, source_ip)
                        .await
                        .map_err(crate::api::error::ApiError::AuthError)?;
                    return Ok(AuthenticatedUser(ctx));
                }
            }
        }

        Err(crate::api::error::ApiError::AuthError(
            crate::auth::types::AuthError::InvalidCredentials,
        ))
    }
}