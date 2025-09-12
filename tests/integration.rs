use kvstore_plus_plus::config::AppConfig;
use kvstore_plus_plus::storage::StorageConfig;
use kvstore_plus_plus::wal::config::WalConfig;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_full_system_integration() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();

    let config = AppConfig {
        storage: StorageConfig {
            num_shards: 4,
            snapshot_dir: data_dir.join("snapshots").to_str().unwrap().to_string(),
        },
        wal: WalConfig {
            dir: data_dir.join("wal").to_str().unwrap().to_string(),
            file_prefix: "wal_".to_string(),
            max_file_size: 1024 * 1024,
            sync_policy: kvstore_plus_plus::wal::config::SyncPolicy::Never,
        },
        background: kvstore_plus_plus::config::BackgroundConfig {
            checkpoint_interval_sec: 60,
            metrics_interval_ms: 1000,
            s3: None,
            replica: None,
        },
    };

    // Initialize WAL
    let wal = Arc::new(kvstore_plus_plus::wal::WalManager::new(config.wal.clone()).await.unwrap());

    // Initialize Storage
    let engine = kvstore_plus_plus::storage::StorageEngine::new(config.storage.clone());

    // Bootstrap catalog
    let _ = kvstore_plus_plus::catalog::bootstrap::bootstrap_if_needed(&engine).await.unwrap();

    // Test SET
    engine
        .set("integration_test", b"success".to_vec(), None)
        .await
        .unwrap();

    // Test GET
    let entry = engine.get("integration_test").await.unwrap();
    assert_eq!(entry.value, b"success");

    // Test DEL
    engine.del("integration_test", None).await.unwrap();
    assert!(engine.get("integration_test").await.is_err());
}