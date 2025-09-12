use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub storage: crate::storage::types::StorageConfig,
    pub wal: crate::wal::config::WalConfig,
    pub background: BackgroundConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundConfig {
    pub checkpoint_interval_sec: u64,
    pub metrics_interval_ms: u64,
    pub s3: Option<S3Config>,
    pub replica: Option<ReplicaConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>, // for MinIO/S3-compatible
    pub upload_after_snapshot: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReplicaConfig {
    pub enabled: bool,
    pub bind_addr: String,
    pub sync_mode: bool, // false = async
}