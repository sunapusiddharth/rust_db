use std::time::SystemTime;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KvEntry {
    pub value: Vec<u8>,
    pub version: u64,
    pub created_at: u64,           // Unix nanos
    pub expires_at: Option<u64>,   // Unix nanos, None = no expiry
}

impl KvEntry {
    pub fn new(value: Vec<u8>, ttl_secs: Option<u64>) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let expires_at = ttl_secs.map(|ttl| now + ttl * 1_000_000_000);

        Self {
            value,
            version: 1,
            created_at: now,
            expires_at,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64;
            now > expiry
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub num_shards: usize,
    pub snapshot_dir: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            num_shards: 256, // power of 2 for fast modulo
            snapshot_dir: "data/snapshots".to_string(),
        }
    }
}