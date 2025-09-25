use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::storage::engine::StorageEngine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtlEvent {
    pub key: String,
    pub expires_at: u64,
}

// For min-heap (earliest expiry first)
impl Ord for TtlEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse because BinaryHeap is max-heap
        other.expires_at.cmp(&self.expires_at)
    }
}

impl PartialOrd for TtlEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Debug)]
pub struct TtlManager {
    engine: Arc<StorageEngine>,
    queue: Arc<Mutex<BinaryHeap<TtlEvent>>>,
}

impl TtlManager {
    pub fn new(engine: Arc<StorageEngine>) -> Self {
        Self {
            engine,
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
        }
    }

    pub async fn add(&self, key: String, expires_at: u64) {
        let mut queue = self.queue.lock().await;
        queue.push(TtlEvent { key, expires_at });
    }

    pub async fn start_background_task(&self) {
        let engine = self.engine.clone();
        let queue = self.queue.clone();

        tokio::spawn(async move {
            loop {
                sleep(Duration::from_millis(100)).await; // check 10x/sec

                let mut to_delete = Vec::new();
                {
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64;

                    let mut queue = queue.lock().await;
                    while let Some(event) = queue.peek().cloned() {
                        if event.expires_at <= now {
                            to_delete.push(queue.pop().unwrap().key);
                        } else {
                            break;
                        }
                    }
                }

                // Delete expired keys
                for key in to_delete {
                    if let Err(e) = engine.del(&key, None).await {
                        tracing::warn!(key = %key, error = %e, "Failed to delete expired key");
                    }
                }
            }
        });
    }
}
