use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::wal::entry::WalEntry;
use crate::wal::error::WalError;

pub struct WalManager {
    config: WalConfig,
    current_file: Mutex<WalFileHandle>,
    sync_task: Option<tokio::task::JoinHandle<()>>,
}

struct WalFileHandle {
    file: File,
    path: PathBuf,
    offset: u64,
}

impl WalManager {
    pub async fn new(config: WalConfig) -> Result<Self, WalError> {
        std::fs::create_dir_all(&config.dir)?;

        let current_file = Self::open_next_file(&config).await?;

        let manager = Self {
            config: config.clone(),
            current_file: Mutex::new(current_file),
            sync_task: None,
        };

        // Start background fsync task if needed
        if let SyncPolicy::EveryMs(interval_ms) = config.sync_policy {
            let manager_clone = Arc::new(manager.clone());
            let handle = tokio::spawn(async move {
                let interval = Duration::from_millis(interval_ms);
                loop {
                    sleep(interval).await;
                    if let Err(e) = manager_clone.sync().await {
                        tracing::error!("WAL sync error: {}", e);
                    }
                }
            });
            manager.sync_task = Some(handle);
        }

        Ok(manager)
    }

    async fn open_next_file(config: &WalConfig) -> Result<WalFileHandle, WalError> {
        let dir = Path::new(&config.dir);
        let mut max_seq = 0u64;

        // Find highest sequence number
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if filename.starts_with(&config.file_prefix) {
                    if let Some(seq_str) = filename.strip_prefix(&config.file_prefix) {
                        if let Ok(seq) = seq_str.parse::<u64>() {
                            max_seq = max_seq.max(seq);
                        }
                    }
                }
            }
        }

        let next_seq = max_seq + 1;
        let filename = format!("{}{}", config.file_prefix, next_seq);
        let path = dir.join(filename);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        let metadata = file.metadata()?;
        let offset = metadata.len();

        tracing::info!(path = %path.display(), offset = offset, "Opened new WAL file");

        Ok(WalFileHandle {
            file,
            path,
            offset,
        })
    }

    pub async fn append(&self, entry: &WalEntry) -> Result<u64, WalError> {
        let serialized = entry.serialize();
        let mut handle = self.current_file.lock().await;

        // Check if we need to rotate
        if handle.offset + serialized.len() as u64 > self.config.max_file_size {
            *handle = Self::open_next_file(&self.config).await?;
        }

        // Write
        handle.file.write_all(&serialized)?;
        let entry_offset = handle.offset;
        handle.offset += serialized.len() as u64;

        // Fsync if policy is EveryWrite
        if let SyncPolicy::EveryWrite = self.config.sync_policy {
            handle.file.sync_all()?;
        }

        tracing::trace!(offset = entry_offset, key = %entry.key, op = ?entry.op_type, "WAL entry appended");

        Ok(entry_offset)
    }

    pub async fn sync(&self) -> Result<(), WalError> {
        let handle = self.current_file.lock().await;
        handle.file.sync_all()?;
        Ok(())
    }

    pub async fn replay_from(
        &self,
        start_offset: u64,
        mut callback: impl FnMut(u64, WalEntry) -> Result<(), WalError>,
    ) -> Result<(), WalError> {
        let handle = self.current_file.lock().await;

        // Seek to start offset
        handle.file.seek(SeekFrom::Start(start_offset))?;

        let mut buf = Vec::new();
        let mut offset = start_offset;

        loop {
            match handle.file.read_to_end(&mut buf) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let mut pos = 0;
                    while pos < buf.len() {
                        match WalEntry::deserialize(&buf[pos..]) {
                            Ok((entry, consumed)) => {
                                callback(offset + pos as u64, entry)?;
                                pos += consumed;
                            }
                            Err(e) => {
                                return Err(WalError::ReplayError {
                                    offset: offset + pos as u64,
                                    reason: e.to_string(),
                                });
                            }
                        }
                    }
                    break;
                }
                Err(e) => return Err(WalError::Io(e)),
            }
        }

        Ok(())
    }

    pub async fn current_offset(&self) -> u64 {
        self.current_file.lock().await.offset
    }
}

impl Drop for WalManager {
    fn drop(&mut self) {
        if let Some(handle) = self.sync_task.take() {
            handle.abort();
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_wal_append_and_replay() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            dir: temp_dir.path().to_str().unwrap().to_string(),
            file_prefix: "test_".to_string(),
            max_file_size: 1024,
            sync_policy: SyncPolicy::Never,
        };

        let wal = WalManager::new(config).await.unwrap();

        // Append entries
        let entry1 = WalEntry {
            timestamp: 1,
            key: "key1".to_string(),
            value: b"value1".to_vec(),
            version: 1,
            ttl: None,
            op_type: OpType::Set,
        };
        let offset1 = wal.append(&entry1).await.unwrap();

        let entry2 = WalEntry {
            timestamp: 2,
            key: "key2".to_string(),
            value: b"value2".to_vec(),
            version: 1,
            ttl: None,
            op_type: OpType::Del,
        };
        let _offset2 = wal.append(&entry2).await.unwrap();

        // Replay
        let mut replayed = Vec::new();
        wal.replay_from(0, |offset, entry| {
            replayed.push((offset, entry));
            Ok(())
        })
        .await
        .unwrap();

        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].0, 0);
        assert_eq!(replayed[0].1.key, "key1");
        assert_eq!(replayed[1].1.key, "key2");
    }

    #[tokio::test]
    async fn test_wal_checksum_validation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            dir: temp_dir.path().to_str().unwrap().to_string(),
            file_prefix: "test_".to_string(),
            max_file_size: 1024,
            sync_policy: SyncPolicy::Never,
        };

        let wal = WalManager::new(config).await.unwrap();

        let entry = WalEntry {
            timestamp: 1,
            key: "key1".to_string(),
            value: b"value1".to_vec(),
            version: 1,
            ttl: None,
            op_type: OpType::Set,
        };
        let _ = wal.append(&entry).await.unwrap();

        // Corrupt the file
        let mut path = temp_dir.path().to_path_buf();
        path.push("test_1");
        let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.seek(std::io::SeekFrom::Start(10)).unwrap();
        file.write_all(&[0xFF]).unwrap();

        // Replay should fail
        let result = wal.replay_from(0, |_offset, _entry| Ok(())).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WalError::ChecksumMismatch { .. }));
    }
}