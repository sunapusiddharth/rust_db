use std::sync::Arc;
use tokio::signal;
use tracing::info;

pub struct ServerHandle {
    rest_handle: tokio::task::JoinHandle<()>,
    grpc_handle: tokio::task::JoinHandle<()>,
    background_workers: crate::background::WorkerManager,
}

impl ServerHandle {
    pub async fn wait_for_shutdown(self) {
        // Wait for Ctrl+C or SIGTERM
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down...");
            }
            _ = signal::unix::signal(signal::unix::SignalKind::terminate()) => {
                info!("Received SIGTERM, shutting down...");
            }
        }

        // Shutdown background workers
        self.background_workers.shutdown();

        // Wait for API servers to shutdown (they don't yet — we’ll add shutdown later)
        let _ = self.rest_handle.await;
        let _ = self.grpc_handle.await;

        info!("Server shutdown complete.");
    }
}