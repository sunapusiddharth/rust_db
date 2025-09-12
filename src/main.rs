use std::sync::Arc;
use tracing::{info, error};

mod kvstore {
    tonic::include_proto!("kvstore");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("KVStore++ starting...");

    // Load config
    let config_str = std::fs::read_to_string("config.toml")
        .unwrap_or_else(|_| include_str!("../default_config.toml").to_string());
    let config: crate::config::AppConfig = toml::from_str(&config_str)?;

    // Create data directories
    std::fs::create_dir_all(&config.wal.dir)?;
    std::fs::create_dir_all(&config.storage.snapshot_dir)?;

    // Initialize WAL
    let wal = Arc::new(crate::wal::WalManager::new(config.wal.clone()).await?);

    // Initialize Storage Engine
    let engine = crate::storage::StorageEngine::new(config.storage.clone());

    // Recover from WAL if needed
    // Placeholder: In MVP, we don't have checkpoint recovery yet
    // Later: load last snapshot + replay WAL from offset

    // Bootstrap system catalog
    let bootstrapped = crate::catalog::bootstrap::bootstrap_if_needed(&engine).await?;
    if bootstrapped {
        info!("System catalog bootstrapped.");
    }

    // Initialize Catalog Manager
    let catalog = Arc::new(crate::catalog::CatalogManager::new(engine.clone()));

    // Initialize Auth Manager
    let auth = Arc::new(crate::auth::AuthManager::new(
        catalog.clone(),
        "my_jwt_secret_123".to_string(), // ⚠️ In production, load from secure config
        "audit.log".to_string(),
    )?);

    // Initialize Background Workers
    let mut background_workers = crate::background::WorkerManager::new(
        engine.clone(),
        wal.clone(),
        &config.background,
    )
    .await?;

    // Start metrics HTTP server (Prometheus endpoint)
    let metrics_addr = "0.0.0.0:9091".parse()?;
    let metrics_engine = engine.clone();
    let metrics_wal = wal.clone();
    tokio::spawn(async move {
        start_metrics_server(metrics_addr, metrics_engine, metrics_wal).await;
    });

    // Start health check server
    let health_addr = "0.0.0.0:9092".parse()?;
    tokio::spawn(async move {
        start_health_server(health_addr).await;
    });

    // Start API servers
    let rest_addr = "0.0.0.0:8080".parse()?;
    let grpc_addr = "0.0.0.0:9090".parse()?;

    let rest_engine = engine.clone();
    let rest_auth = auth.clone();
    let grpc_engine = engine.clone();

    let rest_handle = tokio::spawn(async move {
        crate::api::rest::start_rest_server(rest_addr, rest_engine, rest_auth).await;
    });

    let grpc_handle = tokio::spawn(async move {
        crate::api::grpc::start_grpc_server(grpc_addr, grpc_engine).await;
    });

    // Create server handle for graceful shutdown
    let server_handle = crate::server::ServerHandle {
        rest_handle,
        grpc_handle,
        background_workers,
    };

    info!("KVStore++ ready to accept connections.");
    info!("REST API: http://0.0.0.0:8080");
    info!("gRPC API: http://0.0.0.0:9090");
    info!("Metrics: http://0.0.0.0:9091/metrics");
    info!("Health: http://0.0.0.0:9092/health");

    // Wait for shutdown
    server_handle.wait_for_shutdown().await;

    Ok(())
}

async fn start_metrics_server(
    addr: std::net::SocketAddr,
    engine: Arc<crate::storage::StorageEngine>,
    wal: Arc<crate::wal::WalManager>,
) {
    let app = axum::Router::new()
        .route("/metrics", axum::routing::get(metrics_handler))
        .with_state((engine, wal));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn metrics_handler(
    axum::extract::State((engine, wal)): axum::extract::State<(
        Arc<crate::storage::StorageEngine>,
        Arc<crate::wal::WalManager>,
    )>,
) -> String {
    // Update gauges
    let wal_offset = wal.current_offset().await;
    let key_count = engine
        .shards
        .iter()
        .map(|shard| shard.len())
        .sum::<usize>();

    crate::background::metrics::WAL_SIZE.set(wal_offset as i64);
    crate::background::metrics::KEY_COUNT.set(key_count as i64);

    // Encode all metrics
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap_or_default()
}

async fn start_health_server(addr: std::net::SocketAddr) {
    let app = axum::Router::new().route("/health", axum::routing::get(health_handler));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn health_handler() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "uptime": "running",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}