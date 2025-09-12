use axum::{routing::post, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::Level;

use crate::api::auth_middleware::AuthState;
use crate::auth::AuthManager;
use crate::storage::StorageEngine;

pub async fn start_rest_server(
    addr: SocketAddr,
    engine: Arc<StorageEngine>,
    auth_manager: Arc<AuthManager>,
) {
    let auth_state = AuthState {
        auth_manager: auth_manager.clone(),
    };

    let app = Router::new()
        .route("/v1/get", axum::routing::get(super::handlers::get_handler))
        .route("/v1/set", post(super::handlers::set_handler))
        .route("/v1/del", post(super::handlers::delete_handler))
        .layer(axum::middleware::from_extractor_with_state::<
            super::auth_middleware::AuthenticatedUser,
            _,
        >(auth_state))
        .layer(TraceLayer::new_for_http().make_span_with(|request| {
            tracing::span!(
                Level::INFO,
                "http_request",
                method = %request.method(),
                uri = %request.uri(),
                version = ?request.version(),
            )
        }))
        .with_state(engine);

    tracing::info!("Starting REST server on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}