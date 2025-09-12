use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::storage::StorageEngine;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{Client, Config};
use tokio::sync::oneshot;
use tokio::time::sleep;

use super::types::WorkerError;

pub struct S3Uploader {
    engine: Arc<StorageEngine>,
    snapshot_dir: String,
    bucket: String,
    client: Client,
    upload_after_snapshot: bool,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl S3Uploader {
    pub async fn new(
        engine: Arc<StorageEngine>,
        snapshot_dir: String,
        bucket: String,
        region: String,
        endpoint: Option<String>,
        upload_after_snapshot: bool,
    ) -> Result<Self, WorkerError> {
        let config = if let Some(endpoint) = endpoint {
            Config::builder()
                .region(aws_sdk_s3::config::Region::new(region))
                .endpoint_url(endpoint)
                .build()
        } else {
            aws_config::load_from_env().await.into()
        };

        let client = Client::from_conf(config);

        Ok(Self {
            engine,
            snapshot_dir,
            bucket,
            client,
            upload_after_snapshot,
            shutdown_tx: None,
        })
    }

    pub async fn start(&mut self) -> Result<tokio::task::JoinHandle<()>, WorkerError> {
        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        let snapshot_dir = self.snapshot_dir.clone();
        let bucket = self.bucket.clone();
        let client = self.client.clone();
        let upload_after_snapshot = self.upload_after_snapshot;

        let handle = tokio::spawn(async move {
            let mut last_snapshot = String::new();

            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(10)) => {
                        // List snapshots
                        match fs::read_dir(&snapshot_dir) {
                            Ok(entries) => {
                                let mut snapshots: Vec<_> = entries
                                    .filter_map(|e| e.ok())
                                    .map(|e| e.path())
                                    .filter(|p| p.extension().map_or(false, |ext| ext == "bin"))
                                    .collect();

                                snapshots.sort(); // by name (which includes timestamp)

                                if let Some(latest) = snapshots.last() {
                                    let filename = latest.file_name().unwrap().to_string_lossy().to_string();
                                    if filename != last_snapshot {
                                        last_snapshot = filename.clone();

                                        if upload_after_snapshot {
                                            tracing::info!(filename = %filename, "Uploading snapshot to S3");
                                            match upload_snapshot(&client, &bucket, &snapshot_dir, &filename).await {
                                                Ok(_) => {
                                                    tracing::info!(filename = %filename, "Snapshot uploaded to S3");
                                                }
                                                Err(e) => {
                                                    tracing::error!(filename = %filename, error = %e, "Failed to upload snapshot");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to read snapshot dir: {}", e);
                            }
                        }
                    }
                    _ = rx => {
                        tracing::info!("S3 uploader shutting down");
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

async fn upload_snapshot(
    client: &Client,
    bucket: &str,
    snapshot_dir: &str,
    filename: &str,
) -> Result<(), aws_sdk_s3::Error> {
    let path = PathBuf::from(snapshot_dir).join(filename);
    let body = ByteStream::from_path(&path).await?;

    client
        .put_object()
        .bucket(bucket)
        .key(filename)
        .body(body)
        .send()
        .await?;

    Ok(())
}
