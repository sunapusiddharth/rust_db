use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum UserCommand {
    /// Create a new user
    Create(UserCreateArgs),
    /// List users
    List,
    /// Delete a user
    Delete { username: String },
}

#[derive(Args)]
pub struct UserCreateArgs {
    /// Username
    pub username: String,

    /// Password (will be prompted if not provided)
    #[arg(short, long)]
    password: Option<String>,

    /// Roles to assign (comma-separated)
    #[arg(short, long, default_value = "reader")]
    roles: String,
}

pub async fn run(cmd: UserCommand) -> Result<(), crate::ctl::types::KvCtlError> {
    match cmd {
        UserCommand::Create(args) => {
            println!("Creating user: {}", args.username);
            println!("Roles: {}", args.roles);
            println!("Note: User management not implemented in MVP — requires direct catalog access");
        }
        UserCommand::List => {
            println!("Listing users... (not implemented)");
        }
        UserCommand::Delete { username } => {
            println!("Deleting user: {}", username);
        }
    }
    Ok(())
}