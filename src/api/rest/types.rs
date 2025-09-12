use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GetParams {
    pub key: String,
}

#[derive(Serialize)]
pub struct GetResponse {
    pub found: bool,
    pub value: Option<String>, // base64 for binary? or use bytes
    pub version: u64,
}

#[derive(Deserialize)]
pub struct SetParams {
    pub key: String,
    pub value: String, // base64-encoded
    #[serde(default)]
    pub ttl: Option<u64>, // seconds
}

#[derive(Serialize)]
pub struct SetResponse {
    pub success: bool,
    pub version: u64,
}

#[derive(Deserialize)]
pub struct DeleteParams {
    pub key: String,
}

#[derive(Serialize)]
pub struct DeleteResponse {
    pub success: bool,
}

#[derive(Deserialize)]
pub struct IncrParams {
    pub key: String,
    pub delta: i64,
}

#[derive(Serialize)]
pub struct IncrResponse {
    pub success: bool,
    pub new_value: i64,
}

#[derive(Deserialize)]
pub struct ScanParams {
    pub pattern: String,
    #[serde(default = "default_limit")]
    pub limit: u64,
}

fn default_limit() -> u64 {
    100
}

#[derive(Serialize)]
pub struct ScanItem {
    pub key: String,
    pub value: Option<String>,
    pub version: u64,
}

#[derive(Serialize)]
pub struct ScanResponse {
    pub items: Vec<ScanItem>,
    pub has_more: bool,
}
