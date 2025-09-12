use std::process;

use kvstore::kv_store_client; // generated from proto
use tracing_subscriber;

mod kvstore {
    tonic::include_proto!("kvstore");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = crate::ctl::KvCtl::parse();

    if let Err(e) = cli.run().await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}