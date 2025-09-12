use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{routing::get, Router};
use clap::Parser;
use rand::Rng;
use reqwest;
use tokio::time::sleep;

#[derive(Parser)]
struct Args {
    /// Target KVStore++ URL
    #[arg(short, long, default_value = "http://localhost:8080")]
    target_url: String,

    /// API Key for authentication
    #[arg(short, long)]
    api_key: Option<String>,
}

// Global counters for metrics
static TOTAL_OPS: AtomicU64 = AtomicU64::new(0);
static TOTAL_ERRORS: AtomicU64 = AtomicU64::new(0);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Read env vars
    let ops_per_sec: u64 = env::var("LOAD_OPS_PER_SEC")
        .unwrap_or("1000".to_string())
        .parse()
        .expect("LOAD_OPS_PER_SEC must be a number");

    let get_ratio: f64 = env::var("LOAD_GET_RATIO")
        .unwrap_or("0.6".to_string())
        .parse()
        .expect("LOAD_GET_RATIO must be a float between 0 and 1");

    let set_ratio: f64 = env::var("LOAD_SET_RATIO")
        .unwrap_or("0.3".to_string())
        .parse()
        .expect("LOAD_SET_RATIO must be a float between 0 and 1");

    let del_ratio: f64 = env::var("LOAD_DEL_RATIO")
        .unwrap_or("0.05".to_string())
        .parse()
        .expect("LOAD_DEL_RATIO must be a float between 0 and 1");

    let incr_ratio: f64 = env::var("LOAD_INCR_RATIO")
        .unwrap_or("0.05".to_string())
        .parse()
        .expect("LOAD_INCR_RATIO must be a float between 0 and 1");

    let total_ratio = get_ratio + set_ratio + del_ratio + incr_ratio;
    if (total_ratio - 1.0).abs() > 0.001 {
        eprintln!("Warning: operation ratios don't sum to 1.0 (got {})", total_ratio);
    }

    println!("🚀 Starting dummy load server...");
    println!("Target: {}", args.target_url);
    println!("Ops/sec: {}", ops_per_sec);
    println!("Ratios - GET: {}, SET: {}, DEL: {}, INCR: {}", get_ratio, set_ratio, del_ratio, incr_ratio);

    // Start metrics server
    let metrics_port: u16 = env::var("METRICS_PORT")
        .unwrap_or("9095".to_string())
        .parse()
        .expect("METRICS_PORT must be a number");

    let metrics_addr = format!("0.0.0.0:{}", metrics_port);
    let metrics_total_ops = TOTAL_OPS.clone();
    let metrics_total_errors = TOTAL_ERRORS.clone();

    tokio::spawn(async move {
        let app = Router::new().route("/metrics", get(move || async move {
            format!(
                "# HELP dummy_load_total_ops Total operations performed\n\
                 # TYPE dummy_load_total_ops counter\n\
                 dummy_load_total_ops {}\n\
                 # HELP dummy_load_total_errors Total errors encountered\n\
                 # TYPE dummy_load_total_errors counter\n\
                 dummy_load_total_errors {}\n",
                metrics_total_ops.load(Ordering::Relaxed),
                metrics_total_errors.load(Ordering::Relaxed)
            )
        }));

        println!("📈 Metrics server running on http://{}", metrics_addr);
        axum::Server::bind(&metrics_addr.parse().unwrap())
            .serve(app.into_make_service())
            .await
            .unwrap();
    });

    // Start load generation
    let client = reqwest::Client::new();
    let target_url = args.target_url.clone();
    let api_key = args.api_key.clone();

    let delay_per_op = Duration::from_secs(1) / ops_per_sec;

    println!("⏱️  Generating load with delay: {:?} per op", delay_per_op);

    loop {
        let start = Instant::now();

        // Choose operation based on ratios
        let op_type = {
            let r = rand::thread_rng().gen_range(0.0..1.0);
            if r < get_ratio {
                "GET"
            } else if r < get_ratio + set_ratio {
                "SET"
            } else if r < get_ratio + set_ratio + del_ratio {
                "DEL"
            } else {
                "INCR"
            }
        };

        let key_id = rand::thread_rng().gen_range(0..1_000_000);
        let key = format!("load_test:{}", key_id);

        let result = match op_type {
            "GET" => {
                let mut req = client.get(format!("{}/v1/get?key={}", target_url, key));
                if let Some(ref key) = api_key {
                    req = req.header("X-API-Key", key);
                }
                req.send().await
            }
            "SET" => {
                let value: String = (0..64).map(|_| 'A').collect();
                let mut req = client.post(format!("{}/v1/set", target_url))
                    .json(&serde_json::json!({
                        "key": key,
                        "value": base64::encode(&value),
                        "ttl": 3600
                    }));
                if let Some(ref key) = api_key {
                    req = req.header("X-API-Key", key);
                }
                req.send().await
            }
            "DEL" => {
                let mut req = client.post(format!("{}/v1/del", target_url))
                    .json(&serde_json::json!({
                        "key": key
                    }));
                if let Some(ref key) = api_key {
                    req = req.header("X-API-Key", key);
                }
                req.send().await
            }
            "INCR" => {
                let mut req = client.post(format!("{}/v1/set", target_url)) // Placeholder - use INCR when implemented
                    .json(&serde_json::json!({
                        "key": key,
                        "value": base64::encode(&format!("{}", rand::thread_rng().gen_range(1..100))),
                        "ttl": 3600
                    }));
                if let Some(ref key) = api_key {
                    req = req.header("X-API-Key", key);
                }
                req.send().await
            }
            _ => unreachable!(),
        };

        match result {
            Ok(_) => {
                TOTAL_OPS.fetch_add(1, Ordering::Relaxed);
            }
            Err(_) => {
                TOTAL_ERRORS.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Sleep to maintain target ops/sec
        let elapsed = start.elapsed();
        if elapsed < delay_per_op {
            sleep(delay_per_op - elapsed).await;
        }
    }
}