use clap::Args;

#[derive(Args)]
pub struct KeysArgs {
    /// Key pattern (e.g., "user:*")
    #[arg(short, long)]
    pattern: Option<String>,

    /// Include system keys (_sys.*)
    #[arg(long)]
    include_system: bool,

    /// Limit number of results
    #[arg(short, long, default_value_t = 100)]
    limit: u64,
}

pub async fn run(args: KeysArgs) -> Result<(), crate::ctl::types::KvCtlError> {
    // Placeholder: connect to server and scan
    // In MVP: not implemented — requires SCAN RPC
    println!("Scanning keys with pattern: {:?}", args.pattern);
    println!("Note: SCAN not implemented in gRPC yet — returning empty");

    Ok(())
}