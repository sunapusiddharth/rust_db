use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::error::StorageError),

    #[error("WAL error: {0}")]
    Wal(#[from] crate::wal::error::WalError),

    #[error("S3 error: {0}")]
    S3(#[from] aws_sdk_s3::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Shutdown requested")]
    Shutdown,
}
