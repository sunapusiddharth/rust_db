use clap::Subcommand;

#[derive(Subcommand)]
pub enum SnapshotCommand {
    /// Create a new snapshot
    Create,
    /// List existing snapshots
    List,
    /// Restore from snapshot
    Restore { filename: String },
}

pub async fn run(cmd: SnapshotCommand) -> Result<(), crate::ctl::types::KvCtlError> {
    match cmd {
        SnapshotCommand::Create => {
            println!("Creating snapshot... (not implemented — requires server RPC)");
        }
        SnapshotCommand::List => {
            println!("Listing snapshots... (not implemented — requires server RPC)");
        }
        SnapshotCommand::Restore { filename } => {
            println!("Restoring from {}... (not implemented — requires server RPC)", filename);
        }
    }
    Ok(())
}