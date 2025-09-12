use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("CAS failed: version mismatch for key {key} (expected {expected}, got {got})")]
    CasFailed {
        key: String,
        expected: u64,
        got: u64,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] Box<bincode::ErrorKind>),

    #[error("Concurrency error: {0}")]
    Concurrency(String),
}
