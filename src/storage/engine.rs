use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock as AsyncRwLock;

use crate::storage::shard::Shard;
use crate::storage::ttl::TtlManager;
use crate::storage::types::KvEntry;
use crate::wal::entry::{OpType, WalEntry};

#[derive(Debug)]
pub struct StorageEngine {
    pub shards: Vec<Arc<Shard>>,
    ttl_manager: OnceLock<Arc<TtlManager>>, // We'll add metrics, last_wal_offset, etc. later
}

impl StorageEngine {
    pub async fn new(config: super::types::StorageConfig) -> Arc<Self> {
        let shards: Vec<Arc<Shard>> = (0..config.num_shards)
            .map(|_| Arc::new(Shard::new()))
            .collect();

        let engine = Arc::new(Self {
            shards,
            ttl_manager: OnceLock::new(),
        });

        let ttl_manager = Arc::new(TtlManager::new(engine.clone()));
        ttl_manager.start_background_task().await;
        engine.ttl_manager.set(ttl_manager).unwrap();

        engine
    }

    pub fn ttl_manager(&self) -> &TtlManager {
        self.ttl_manager.get().expect("TTL manager not initialized")
    }

    fn get_shard(&self, key: &str) -> &Arc<Shard> {
        let hash = fxhash::hash32(key.as_bytes());
        &self.shards[(hash as usize) % self.shards.len()]
    }

    pub async fn get(&self, key: &str) -> Result<KvEntry, super::error::StorageError> {
        let shard = self.get_shard(key);
        if let Some(entry) = shard.get(key) {
            if entry.is_expired() {
                shard.del(key);
                return Err(super::error::StorageError::KeyNotFound(key.to_string()));
            }
            Ok(entry)
        } else {
            Err(super::error::StorageError::KeyNotFound(key.to_string()))
        }
    }

    pub async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl_secs: Option<u64>,
    ) -> Result<(), super::error::StorageError> {
        let shard = self.get_shard(key);
        let entry = KvEntry::new(value, ttl_secs);

        // Set in shard
        let old_entry = shard.set(key.to_string(), entry.clone());

        // If TTL set, register with TTL manager
        if let Some(expiry) = entry.expires_at {
            self.ttl_manager
                .get()
                .unwrap()
                .add(key.to_string(), expiry)
                .await;
        }

        // If replacing old entry with TTL, remove from TTL manager? (optional optimization)

        Ok(())
    }

    pub async fn del(
        &self,
        key: &str,
        _expected_version: Option<u64>,
    ) -> Result<(), super::error::StorageError> {
        let shard = self.get_shard(key);
        if shard.del(key).is_some() {
            Ok(())
        } else {
            Err(super::error::StorageError::KeyNotFound(key.to_string()))
        }
    }

    pub async fn exists(&self, key: &str) -> bool {
        let shard = self.get_shard(key);
        shard.exists(key) && !shard.get(key).map_or(false, |e| e.is_expired())
    }

    pub async fn apply_wal_entry(
        &self,
        entry: &WalEntry,
    ) -> Result<(), super::error::StorageError> {
        match entry.op_type {
            OpType::Set => {
                self.set(&entry.key, entry.value.clone(), entry.ttl).await?;
            }
            OpType::Del => {
                self.del(&entry.key, None).await?;
            }
            OpType::Incr => {
                // For now, treat as SET — we'll add atomic INCR later
                self.set(&entry.key, entry.value.clone(), entry.ttl).await?;
            }
            OpType::Cas => {
                // For now, treat as SET — we'll add version check later
                self.set(&entry.key, entry.value.clone(), entry.ttl).await?;
            }
        }
        Ok(())
    }

    pub async fn snapshot(&self) -> Vec<HashMap<String, KvEntry>> {
        self.shards.iter().map(|shard| shard.snapshot()).collect()
    }

    pub async fn load_from_snapshot(&self, state: Vec<HashMap<String, KvEntry>>) {
        assert_eq!(state.len(), self.shards.len());

        for (shard, shard_state) in self.shards.iter().zip(state) {
            let mut map = shard.map.write();
            *map = shard_state;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_storage_get_set_del() {
        let config = StorageConfig {
            num_shards: 4,
            snapshot_dir: "test_snapshots".to_string(),
        };
        let engine = StorageEngine::new(config);

        // Set
        engine.set("hello", b"world".to_vec(), None).await.unwrap();

        // Get
        let entry = engine.get("hello").await.unwrap();
        assert_eq!(entry.value, b"world");

        // Del
        engine.del("hello", None).await.unwrap();
        let result = engine.get("hello").await;
        assert!(matches!(result.unwrap_err(), StorageError::KeyNotFound(_)));
    }

    #[tokio::test]
    async fn test_storage_ttl_expiry() {
        let config = StorageConfig {
            num_shards: 4,
            snapshot_dir: "test_snapshots".to_string(),
        };
        let engine = StorageEngine::new(config);

        // Set with 1s TTL
        engine
            .set("temp", b"expiring".to_vec(), Some(1))
            .await
            .unwrap();

        // Should exist now
        assert!(engine.get("temp").await.is_ok());

        // Wait 2s
        sleep(Duration::from_secs(2)).await;

        // Should be expired
        let result = engine.get("temp").await;
        assert!(matches!(result.unwrap_err(), StorageError::KeyNotFound(_)));
    }

    #[tokio::test]
    async fn test_storage_sharding() {
        let config = StorageConfig {
            num_shards: 4,
            snapshot_dir: "test_snapshots".to_string(),
        };
        let engine = StorageEngine::new(config);

        // Set keys
        for i in 0..100 {
            let key = format!("key_{}", i);
            engine
                .set(&key, format!("value_{}", i).into_bytes(), None)
                .await
                .unwrap();
        }

        // Check all keys exist
        for i in 0..100 {
            let key = format!("key_{}", i);
            let entry = engine.get(&key).await.unwrap();
            assert_eq!(entry.value, format!("value_{}", i).into_bytes());
        }
    }
}
