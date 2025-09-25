use std::sync::Arc;
use std::time::Duration;

use prometheus::{register_int_gauge, IntGauge};
use tokio::sync::oneshot;
use tokio::time::sleep;

use crate::storage::StorageEngine;
use crate::wal::WalManager;

use super::types::WorkerError;

lazy_static::lazy_static! {
    static ref WAL_SIZE: IntGauge = register_int_gauge!(
        "kvstore_wal_size_bytes",
        "Current WAL size in bytes"
    ).unwrap();

    static ref MEMORY_USAGE: IntGauge = register_int_gauge!(
        "kvstore_memory_usage_bytes",
        "Estimated memory usage"
    ).unwrap();

    static ref KEY_COUNT: IntGauge = register_int_gauge!(
        "kvstore_key_count",
        "Total number of keys"
    ).unwrap();
}

pub struct MetricsWorker {
    engine: Arc<StorageEngine>,
    wal: Arc<WalManager>,
    interval: Duration,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MetricsWorker {
    pub fn new(engine: Arc<StorageEngine>, wal: Arc<WalManager>, interval_ms: u64) -> Self {
        Self {
            engine,
            wal,
            interval: Duration::from_millis(interval_ms),
            shutdown_tx: None,
        }
    }

    pub async fn start(&mut self) -> Result<tokio::task::JoinHandle<()>, WorkerError> {
        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        let engine = self.engine.clone();
        let wal = self.wal.clone();
        let interval_clone = self.interval.clone();

        let handle = tokio::spawn(async move {
            tokio::pin!(rx); // Pin the receiver so it can be polled multiple times

            loop {
                tokio::select! {
                    _ = sleep(interval_clone) => {
                        let wal_offset = wal.current_offset().await;
                        WAL_SIZE.set(wal_offset as i64);

                        let key_count = engine
                            .shards
                            .iter()
                            .map(|shard| shard.len())
                            .sum::<usize>();
                        KEY_COUNT.set(key_count as i64);

                        MEMORY_USAGE.set((key_count * 100) as i64);
                    }
                    _ = &mut rx => {
                        tracing::info!("Metrics worker shutting down");
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
