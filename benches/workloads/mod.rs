use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadResult {
    pub workload_type: String,
    pub total_ops: u64,
    pub duration_sec: f64,
    pub ops_per_sec: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub error_rate: f64,
}

#[async_trait::async_trait]
pub trait Workload {
    async fn run(&self, client: &Client, concurrency: usize, duration: std::time::Duration) -> WorkloadResult;
}

#[derive(Clone)]
pub struct Client {
    base_url: String,
    api_key: Option<String>,
}

impl Client {
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self { base_url, api_key }
    }

    pub async fn get(&self, key: &str) -> Result<reqwest::Response, reqwest::Error> {
        let client = reqwest::Client::new();
        let mut req = client.get(format!("{}/v1/get?key={}", self.base_url, key));
        if let Some(api_key) = &self.api_key {
            req = req.header("X-API-Key", api_key);
        }
        req.send().await
    }

    pub async fn set(&self, key: &str, value: &str, ttl: Option<u64>) -> Result<reqwest::Response, reqwest::Error> {
        let client = reqwest::Client::new();
        let mut req = client.post(format!("{}/v1/set", self.base_url))
            .json(&serde_json::json!({
                "key": key,
                "value": base64::encode(value),
                "ttl": ttl
            }));
        if let Some(api_key) = &self.api_key {
            req = req.header("X-API-Key", api_key);
        }
        req.send().await
    }
}