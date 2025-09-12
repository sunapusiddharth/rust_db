use thiserror::Error;

#[derive(Error, Debug)]
pub enum CatalogError {
    #[error("System key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid system key format: {0}")]
    InvalidKeyFormat(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::error::StorageError),

    #[error("Password error: {0}")]
    Password(String),
}