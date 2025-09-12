pub mod auth_middleware;
pub mod error;
pub mod grpc;
pub mod rest;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task;

use crate::auth::AuthManager;
use crate::storage::StorageEngine;

pub async fn start_servers(
    rest_addr: SocketAddr,
    grpc_addr: SocketAddr,
    engine: Arc<StorageEngine>,
    auth_manager: Arc<AuthManager>,
) {
    let engine_clone = engine.clone();
    let auth_manager_clone = auth_manager.clone();

    // Start REST server
    task::spawn(async move {
        super::rest::start_rest_server(rest_addr, engine, auth_manager).await;
    });

    // Start gRPC server
    task::spawn(async move {
        super::grpc::start_grpc_server(grpc_addr, engine_clone).await;
    });
}