use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::storage::StorageEngine;
use crate::wal::entry::WalEntry;

pub struct ReplicaStreamer {
    engine: Arc<StorageEngine>,
    bind_addr: String,
    sync_mode: bool,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ReplicaStreamer {
    pub fn new(
        engine: Arc<StorageEngine>,
        bind_addr: String,
        sync_mode: bool,
    ) -> Self {
        Self {
            engine,
            bind_addr,
            sync_mode,
            shutdown_tx: None,
        }
    }

    pub async fn start(&mut self) -> Result<tokio::task::JoinHandle<()>, crate::background::types::WorkerError> {
        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        let engine = self.engine.clone();
        let bind_addr = self.bind_addr.clone();
        let sync_mode = self.sync_mode;

        let handle = tokio::spawn(async move {
            let listener = TcpListener::bind(&bind_addr).await
                .map_err(|e| crate::background::types::WorkerError::Io(e))?;

            tracing::info!("Replica streamer listening on {}", bind_addr);

            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                tracing::info!("Replica connection from {}", addr);
                                
                                let engine = engine.clone();
                                
                                tokio::spawn(async move {
                                    handle_replica_connection(stream, engine, sync_mode).await;
                                });
                            }
                            Err(e) => {
                                tracing::error!("Replica accept error: {}", e);
                            }
                        }
                    }
                    _ = rx => {
                        tracing::info!("Replica streamer shutting down");
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

async fn handle_replica_connection(
    mut stream: tokio::net::TcpStream,
    engine: Arc<StorageEngine>,
    sync_mode: bool,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buffer = Vec::new();
    let mut pos = 0;

    loop {
        // Read data
        let mut temp_buf = [0u8; 1024];
        match stream.read(&mut temp_buf).await {
            Ok(0) => break, // EOF
            Ok(n) => {
                buffer.extend_from_slice(&temp_buf[..n]);
            }
            Err(e) => {
                tracing::error!("Replica read error: {}", e);
                break;
            }
        }

        // Process complete WAL entries
        while pos < buffer.len() {
            if buffer.len() - pos < 8 { // min header size
                break;
            }

            // Read entry size (first 8 bytes)
            let entry_size = u64::from_le_bytes([
                buffer[pos], buffer[pos + 1], buffer[pos + 2], buffer[pos + 3],
                buffer[pos + 4], buffer[pos + 5], buffer[pos + 6], buffer[pos + 7],
            ]) as usize;

            if buffer.len() - pos < 8 + entry_size {
                break; // need more data
            }

            // Extract WAL entry
            let entry_data = &buffer[pos + 8..pos + 8 + entry_size];
            pos += 8 + entry_size;

            match crate::wal::entry::WalEntry::deserialize(entry_data) {
                Ok((entry, _)) => {
                    match engine.apply_wal_entry(&entry).await {
                        Ok(_) => {
                            if sync_mode {
                                // Send ACK back to primary
                                let _ = stream.write_all(b"ACK").await;
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to apply WAL entry: {}", e);
                            let _ = stream.write_all(b"ERR").await;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to deserialize WAL entry: {}", e);
                    let _ = stream.write_all(b"ERR").await;
                    break;
                }
            }
        }

        // Compact buffer
        if pos > 0 {
            buffer.drain(..pos);
            pos = 0;
        }
    }
}