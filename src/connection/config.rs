use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionConfig {
    pub max_connections: usize,
    pub idle_timeout_sec: u64,
    pub evict_policy: String, // "idle_then_priority" | "fifo" | "priority_then_idle"

    #[serde(default)]
    pub per_role: std::collections::HashMap<String, RoleConnectionConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoleConnectionConfig {
    #[serde(default)]
    pub max_connections: Option<usize>,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_sec: u64,
}

fn default_idle_timeout() -> u64 {
    300 // 5 minutes
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            idle_timeout_sec: 300,
            evict_policy: "idle_then_priority".to_string(),
            per_role: std::collections::HashMap::new(),
        }
    }
}