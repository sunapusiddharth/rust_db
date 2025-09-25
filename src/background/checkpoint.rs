use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::sleep;

use crate::storage::StorageEngine;
use crate::wal::WalManager;

use super::types::WorkerError;

pub struct CheckpointWorker {
    engine: Arc<StorageEngine>,
    wal: Arc<WalManager>,
    snapshot_dir: String,
    interval: Duration,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl CheckpointWorker {
    pub fn new(
        engine: Arc<StorageEngine>,
        wal: Arc<WalManager>,
        snapshot_dir: String,
        interval_sec: u64,
    ) -> Self {
        Self {
            engine,
            wal,
            snapshot_dir,
            interval: Duration::from_secs(interval_sec),
            shutdown_tx: None,
        }
    }

    pub async fn start(&mut self) -> Result<tokio::task::JoinHandle<()>, WorkerError> {
        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        let engine = self.engine.clone();
        let wal = self.wal.clone();
        let snapshot_dir = self.snapshot_dir.clone();
        let interval = self.interval;

        let handle = tokio::spawn(async move {
            let snapshot_manager = crate::storage::snapshot::SnapshotManager::new(snapshot_dir);
            tokio::pin!(rx); // Pin the receiver so it can be polled multiple times
            loop {
                tokio::select! {
                    _ = sleep(interval) => {
                        tracing::info!("Starting checkpoint...");

                        // Create snapshot
                        match snapshot_manager.create_snapshot(&engine).await {
                            Ok(filename) => {
                                tracing::info!(filename = %filename, "Snapshot created");

                                // Get current WAL offset
                                let wal_offset = wal.current_offset().await;

                                // Record checkpoint (in a real system, write to pg_control)
                                // For now, just log
                                tracing::info!(wal_offset = wal_offset, "Checkpoint recorded");

                                // Optional: truncate old WAL files (not implemented here)
                            }
                            Err(e) => {
                                tracing::error!("Failed to create snapshot: {}", e);
                            }
                        }
                    }
                    _ = &mut rx => {
                        tracing::info!("Checkpoint worker shutting down");
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }

    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}
