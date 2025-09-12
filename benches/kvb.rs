use clap::{Parser, Subcommand};
use std::time::Duration;

mod reporter;
mod workloads;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run GET-heavy workload
    GetHeavy {
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
        #[arg(short, long)]
        api_key: Option<String>,
        #[arg(short, long, default_value = "test:")]
        key_prefix: String,
        #[arg(short, long, default_value_t = 10000)]
        key_count: usize,
        #[arg(short, long, default_value_t = 10)]
        concurrency: usize,
        #[arg(short, long, default_value_t = 60)]
        duration: u64,
    },
    /// Run SET-heavy workload
    SetHeavy {
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
        #[arg(short, long)]
        api_key: Option<String>,
        #[arg(short, long, default_value = "test:")]
        key_prefix: String,
        #[arg(short, long, default_value_t = 64)]
        value_size: usize,
        #[arg(short, long, default_value_t = 10)]
        concurrency: usize,
        #[arg(short, long, default_value_t = 60)]
        duration: u64,
    },
    /// Run mixed workload
    Mixed {
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
        #[arg(short, long)]
        api_key: Option<String>,
        #[arg(short, long, default_value = "test:")]
        key_prefix: String,
        #[arg(short, long, default_value_t = 64)]
        value_size: usize,
        #[arg(short, long, default_value_t = 0.7)]
        read_ratio: f64,
        #[arg(short, long, default_value_t = 10)]
        concurrency: usize,
        #[arg(short, long, default_value_t = 60)]
        duration: u64,
    },
    /// Run comprehensive benchmark suite
    Suite {
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
        #[arg(short, long)]
        api_key: Option<String>,
        #[arg(short, long, default_value_t = 10)]
        concurrency: usize,
        #[arg(short, long, default_value_t = 30)]
        duration: u64,
        #[arg(short, long, default_value = "benchmark")]
        output_prefix: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut results = Vec::new();

    match cli.command {
        Commands::GetHeavy { url, api_key, key_prefix, key_count, concurrency, duration } => {
            let client = workloads::Client::new(url, api_key);
            let workload = workloads::GetHeavyWorkload { key_prefix, key_count };
            let result = workload.run(&client, concurrency, Duration::from_secs(duration)).await;
            results.push(result);
        }
        Commands::SetHeavy { url, api_key, key_prefix, value_size, concurrency, duration } => {
            let client = workloads::Client::new(url, api_key);
            let workload = workloads::SetHeavyWorkload { key_prefix, value_size_bytes: value_size };
            let result = workload.run(&client, concurrency, Duration::from_secs(duration)).await;
            results.push(result);
        }
        Commands::Mixed { url, api_key, key_prefix, value_size, read_ratio, concurrency, duration } => {
            let client = workloads::Client::new(url, api_key);
            let workload = workloads::MixedWorkload { 
                key_prefix, 
                value_size_bytes: value_size, 
                read_write_ratio: read_ratio 
            };
            let result = workload.run(&client, concurrency, Duration::from_secs(duration)).await;
            results.push(result);
        }
        Commands::Suite { url, api_key, concurrency, duration, output_prefix } => {
            let client = workloads::Client::new(url.clone(), api_key.clone());

            // Test 1: GET-heavy
            println!("Running GET-heavy workload...");
            let get_workload = workloads::GetHeavyWorkload { 
                key_prefix: "get_test:".to_string(), 
                key_count: 10000 
            };
            let get_result = get_workload.run(&client, concurrency, Duration::from_secs(duration)).await;
            results.push(get_result);

            // Test 2: SET-heavy
            println!("Running SET-heavy workload...");
            let set_workload = workloads::SetHeavyWorkload { 
                key_prefix: "set_test:".to_string(), 
                value_size_bytes: 64 
            };
            let set_result = set_workload.run(&client, concurrency, Duration::from_secs(duration)).await;
            results.push(set_result);

            // Test 3: Mixed 70/30
            println!("Running Mixed 70/30 workload...");
            let mixed_workload = workloads::MixedWorkload { 
                key_prefix: "mixed_test:".to_string(), 
                value_size_bytes: 64, 
                read_write_ratio: 0.7 
            };
            let mixed_result = mixed_workload.run(&client, concurrency, Duration::from_secs(duration)).await;
            results.push(mixed_result);

            // Save reports
            reporter::save_json_report(&results, &format!("{}.json", output_prefix))?;
            reporter::save_csv_report(&results, &format!("{}.csv", output_prefix))?;
        }
    }

    // Print report
    reporter::print_text_report(&results);

    Ok(())
}