use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;

use crate::storage::StorageEngine;

pub async fn start_grpc_server(addr: SocketAddr, engine: Arc<StorageEngine>) {
    let svc = kvstore::kv_store_server::KvStoreServer::new(super::service::KvStoreService::new(engine));

    tracing::info!("Starting gRPC server on {}", addr);

    Server::builder()
        .add_service(svc)
        .serve(addr)
        .await
        .unwrap();
}