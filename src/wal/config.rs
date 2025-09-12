use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub enum SyncPolicy {
    EveryWrite,
    EveryMs(u64),
    Never,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalConfig {
    pub dir: String,
    pub file_prefix: String,
    pub max_file_size: u64, // bytes
    pub sync_policy: SyncPolicy,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            dir: "data/wal".to_string(),
            file_prefix: "wal_".to_string(),
            max_file_size: 128 * 1024 * 1024, // 128 MB
            sync_policy: SyncPolicy::EveryMs(100),
        }
    }
}