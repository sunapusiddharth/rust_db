use thiserror::Error;

#[derive(Error, Debug)]
pub enum KvCtlError {
    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::transport::Error),

    #[error("RPC error: {0}")]
    Rpc(#[from] tonic::Status),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}