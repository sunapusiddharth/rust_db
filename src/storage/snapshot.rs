use std::fs::{File, OpenOptions};
use std::path::Path;

use crate::storage::engine::StorageEngine;
use crate::storage::types::KvEntry;
use std::io::Read;
use std::io::Write;
pub struct SnapshotManager {
    snapshot_dir: String,
}

impl SnapshotManager {
    pub fn new(snapshot_dir: String) -> Self {
        std::fs::create_dir_all(&snapshot_dir).ok();
        Self { snapshot_dir }
    }

    pub async fn create_snapshot(
        &self,
        engine: &StorageEngine,
    ) -> Result<String, crate::storage::error::StorageError> {
        use tokio::task;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let filename = format!("snapshot_{}.bin", now);
        let path = Path::new(&self.snapshot_dir).join(&filename);

        // Serialize entire state
        let state = engine.snapshot().await;
        let serialized = task::spawn_blocking(move || bincode::serialize(&state))
            .await
            .map_err(|e| {
                crate::storage::error::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                ))
            })?
            .map_err(|e| crate::storage::error::StorageError::Serialization(e))?;

        // Write to file
        let path_clone = path.clone();
        task::spawn_blocking(move || {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path_clone)?;

            file.write_all(&serialized)?;
            file.flush()?;
            Ok::<(), std::io::Error>(())
        })
        .await
        .map_err(|e| {
            crate::storage::error::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            ))
        })??;

        tracing::info!(path = %path.display(), "Snapshot created");

        Ok(filename)
    }

    pub async fn load_snapshot(
        &self,
        engine: &StorageEngine,
        filename: &str,
    ) -> Result<(), crate::storage::error::StorageError> {
        use tokio::task;

        let path = Path::new(&self.snapshot_dir).join(filename);
        if !path.exists() {
            return Err(crate::storage::error::StorageError::Io(
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Snapshot not found: {}", filename),
                ),
            ));
        }

        let path_clone = path.clone();
        let buffer = task::spawn_blocking(move || {
            let mut file = File::open(&path_clone)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            Ok::<Vec<u8>, std::io::Error>(buffer)
        })
        .await
        .map_err(|e| {
            crate::storage::error::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            ))
        })??;

        let state: Vec<std::collections::HashMap<String, KvEntry>> =
            task::spawn_blocking(move || bincode::deserialize(&buffer))
                .await
                .map_err(|e| {
                    crate::storage::error::StorageError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e,
                    ))
                })?
                .map_err(crate::storage::error::StorageError::Serialization)?;

        engine.load_from_snapshot(state).await;

        tracing::info!(path = %path.display(), "Snapshot loaded");

        Ok(())
    }
}
