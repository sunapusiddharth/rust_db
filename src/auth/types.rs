use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub source_ip: IpAddr,
    pub auth_method: AuthMethod,
    pub session_id: String, // for JWT sessions
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    ApiKey(String), // key ID
    Jwt(String),    // token
    Password,       // for CLI/login
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("User inactive")]
    UserInactive,

    #[error("Account expired")]
    AccountExpired,

    #[error("Permission denied: {0} not allowed for user {1}")]
    PermissionDenied(String, String), // op, user

    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    #[error("Catalog error: {0}")]
    CatalogError(#[from] crate::catalog::error::CatalogError),

    #[error("Storage error: {0}")]
    StorageError(#[from] crate::storage::error::StorageError),
}