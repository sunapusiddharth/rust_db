pub mod client;
pub mod commands;
pub mod types;

use clap::{Parser, Subcommand};

use self::commands::snapshot::SnapshotCommand;
use self::commands::user::UserCommand;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct KvCtl {
    /// gRPC server address
    #[arg(short, long, default_value = "http://[::1]:9090")]
    server: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Inspect keys
    Keys(commands::keys::KeysArgs),

    /// Inspect WAL
    Wal(commands::wal::WalArgs),

    /// Manage snapshots
    Snapshot(SnapshotCommand),

    /// Manage users
    User(UserCommand),
}

impl KvCtl {
    pub async fn run(self) -> Result<(), crate::ctl::types::KvCtlError> {
        match self.command {
            Commands::Keys(args) => commands::keys::run(args).await,
            Commands::Wal(args) => commands::wal::run(args).await,
            Commands::Snapshot(cmd) => commands::snapshot::run(cmd).await,
            Commands::User(cmd) => commands::user::run(cmd).await,
        }
    }
}
