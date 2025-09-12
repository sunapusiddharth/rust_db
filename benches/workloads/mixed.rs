use super::*;
use rand::Rng;
use std::time::Instant;

pub struct MixedWorkload {
    pub key_prefix: String,
    pub value_size_bytes: usize,
    pub read_write_ratio: f64, // 0.7 = 70% reads, 30% writes
}

#[async_trait::async_trait]
impl Workload for MixedWorkload {
    async fn run(&self, client: &Client, concurrency: usize, duration: std::time::Duration) -> WorkloadResult {
        let start = Instant::now();
        let mut latencies = Vec::new();
        let mut errors = 0;
        let mut total_ops = 0;

        let handles: Vec<_> = (0..concurrency)
            .map(|_| {
                let client = client.clone();
                let key_prefix = self.key_prefix.clone();
                let value_size = self.value_size_bytes;
                let ratio = self.read_write_ratio;

                tokio::spawn(async move {
                    let mut local_latencies = Vec::new();
                    let mut local_errors = 0;
                    let mut local_ops = 0;

                    let start = Instant::now();
                    while start.elapsed() < duration {
                        let is_read = rand::thread_rng().gen_bool(ratio);

                        let op_start = Instant::now();
                        let result = if is_read {
                            let key_id = rand::thread_rng().gen_range(0..1_000_000);
                            let key = format!("{}{}", key_prefix, key_id);
                            client.get(&key).await
                        } else {
                            let key_id = rand::thread_rng().gen_range(0..1_000_000);
                            let key = format!("{}{}", key_prefix, key_id);
                            let value: String = (0..value_size).map(|_| 'A').collect();
                            client.set(&key, &value, None).await
                        };

                        match result {
                            Ok(_) => {
                                local_latencies.push(op_start.elapsed().as_millis() as f64);
                                local_ops += 1;
                            }
                            Err(_) => {
                                local_errors += 1;
                            }
                        }
                    }

                    (local_latencies, local_errors, local_ops)
                })
            })
            .collect();

        for handle in handles {
            let (lats, errs, ops) = handle.await.unwrap();
            latencies.extend(lats);
            errors += errs;
            total_ops += ops;
        }

        let duration_sec = start.elapsed().as_secs_f64();
        let ops_per_sec = total_ops as f64 / duration_sec;

        // Calculate percentiles
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50 = latencies.get((latencies.len() as f64 * 0.5) as usize).copied().unwrap_or(0.0);
        let p95 = latencies.get((latencies.len() as f64 * 0.95) as usize).copied().unwrap_or(0.0);
        let p99 = latencies.get((latencies.len() as f64 * 0.99) as usize).copied().unwrap_or(0.0);

        let error_rate = if total_ops + errors > 0 {
            errors as f64 / (total_ops + errors) as f64
        } else {
            0.0
        };

        WorkloadResult {
            workload_type: format!("Mixed ({:.0}% read)", self.read_write_ratio * 100.0),
            total_ops,
            duration_sec,
            ops_per_sec,
            latency_p50_ms: p50,
            latency_p95_ms: p95,
            latency_p99_ms: p99,
            error_rate,
        }
    }
}