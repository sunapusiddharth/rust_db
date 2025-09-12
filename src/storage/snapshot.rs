use std::fs::{File, OpenOptions};
use std::path::Path;

use crate::storage::engine::StorageEngine;
use crate::storage::types::KvEntry;

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
        let serialized = task::spawn_blocking(move || bincode::serialize(&state)).await?;

        // Write to file
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;

        task::spawn_blocking(move || {
            std::io::Write::write_all(&file, &serialized)?;
            std::io::Write::flush(file)?;
            Result::<(), std::io::Error>::Ok(())
        })
        .await?;

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

        let file = File::open(&path)?;
        let metadata = file.metadata()?;
        let mut buffer = Vec::with_capacity(metadata.len() as usize);
        task::spawn_blocking(move || {
            std::io::Read::read_to_end(&file, &mut buffer)?;
            Result::<(), std::io::Error>::Ok(())
        })
        .await?;

        let state: Vec<std::collections::HashMap<String, KvEntry>> =
            task::spawn_blocking(move || {
                bincode::deserialize(&buffer).map_err(|e| Box::new(e) as Box<bincode::ErrorKind>)
            })
            .await?;

        // Load into engine
        engine.load_from_snapshot(state).await;

        tracing::info!(path = %path.display(), "Snapshot loaded");

        Ok(())
    }
}
