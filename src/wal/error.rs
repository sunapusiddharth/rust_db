use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum WalError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid WAL entry at offset {offset}: {reason}")]
    InvalidEntry { offset: u64, reason: String },

    #[error("Checksum mismatch at offset {offset}: expected {expected}, got {got}")]
    ChecksumMismatch { offset: u64, expected: u32, got: u32 },

    #[error("WAL file not found: {0}")]
    FileNotFound(String),

    #[error("Replay stopped at offset {offset}: {reason}")]
    ReplayError { offset: u64, reason: String },
}