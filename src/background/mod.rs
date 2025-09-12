pub mod checkpoint;
pub mod metrics;
pub mod replica;
pub mod s3_uploader;
pub mod types;

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::config;
use crate::storage::StorageEngine;
use crate::wal::WalManager;

pub struct WorkerManager {
    checkpoint: Option<checkpoint::CheckpointWorker>,
    metrics: Option<metrics::MetricsWorker>,
    s3_uploader: Option<s3_uploader::S3Uploader>,
    replica: Option<replica::ReplicaStreamer>,
}

impl WorkerManager {
    pub async fn new(
        engine: Arc<StorageEngine>,
        wal: Arc<WalManager>,
        config: &crate::config::BackgroundConfig,
    ) -> Result<Self, crate::background::types::WorkerError> {
        let mut manager = Self {
            checkpoint: None,
            metrics: None,
            s3_uploader: None,
            replica: None,
        };

        // Start checkpoint worker
        let mut checkpoint_worker = checkpoint::CheckpointWorker::new(
            engine.clone(),
            wal.clone(),
            config::AppConfig::default().storage.snapshot_dir.clone(),
            config.checkpoint_interval_sec,
        );
        let _checkpoint_handle = checkpoint_worker.start().await?;
        manager.checkpoint = Some(checkpoint_worker);

        // Start metrics worker
        let mut metrics_worker =
            metrics::MetricsWorker::new(engine.clone(), wal.clone(), config.metrics_interval_ms);
        let _metrics_handle = metrics_worker.start().await?;
        manager.metrics = Some(metrics_worker);

        // Start S3 uploader if configured
        if let Some(s3_config) = &config.s3 {
            let mut s3_uploader = s3_uploader::S3Uploader::new(
                engine.clone(),
                config::AppConfig::default().storage.snapshot_dir.clone(),
                s3_config.bucket.clone(),
                s3_config.region.clone(),
                s3_config.endpoint.clone(),
                s3_config.upload_after_snapshot,
            )
            .await?;
            let _s3_handle = s3_uploader.start().await?;
            manager.s3_uploader = Some(s3_uploader);
        }

        Ok(manager)
    }

    pub fn shutdown(&mut self) {
        if let Some(worker) = &mut self.checkpoint {
            worker.shutdown();
        }
        if let Some(worker) = &mut self.metrics {
            worker.shutdown();
        }
        if let Some(worker) = &mut self.s3_uploader {
            worker.shutdown();
        }
        if let Some(worker) = &mut self.replica {
            // ← ADDED
            worker.shutdown();
        }
    }
}
