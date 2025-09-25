use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::wal::entry::WalEntry;
use crate::wal::error::WalError;

use super::config::SyncPolicy;
use super::WalConfig;

#[derive(Debug)]
pub struct WalManager {
    config: WalConfig,
    current_file: Mutex<WalFileHandle>,
    sync_task: Option<tokio::task::JoinHandle<()>>,
}
#[derive(Debug)]
struct WalFileHandle {
    file: File,
    path: PathBuf,
    offset: u64,
}

impl WalManager {
    pub async fn new(config: WalConfig) -> Result<Arc<Self>, WalError> {
        std::fs::create_dir_all(&config.dir)?;

        let current_file = Self::open_next_file(&config).await?;

        let manager = Arc::new(Self {
            config: config.clone(),
            current_file: Mutex::new(current_file),
            sync_task: None,
        });

        // Start background fsync task if needed
        if let SyncPolicy::EveryMs(interval_ms) = config.sync_policy {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                let interval = Duration::from_millis(interval_ms);
                loop {
                    sleep(interval).await;
                    if let Err(e) = manager_clone.sync().await {
                        tracing::error!("WAL sync error: {}", e);
                    }
                }
            });

            // Use interior mutability to store the handle
            Arc::get_mut(&mut Arc::clone(&manager))
                .expect("No other Arc references exist")
                .sync_task = Some(handle);
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

        Ok(WalFileHandle { file, path, offset })
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
        let mut handle = self.current_file.lock().await;

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
