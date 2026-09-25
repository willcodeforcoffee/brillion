use clap::{Parser, Subcommand};

/// Brillion — a federated ActivityPub blog.
#[derive(Parser)]
#[command(name = "brillion", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the HTTP server (GraphQL + ActivityPub + public blog pages).
    Serve,
    /// Run pending database migrations.
    Migrate,
    /// Manage user accounts.
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Manage blocked remote domains.
    DomainBlock {
        #[command(subcommand)]
        command: DomainBlockCommand,
    },
}

#[derive(Subcommand)]
pub enum UserCommand {
    /// Create a new user (and their local actor).
    Create {
        #[arg(long)]
        email: String,
        #[arg(long)]
        username: String,
        #[arg(long, default_value = "author")]
        role: String,
        /// Read the password from stdin instead of prompting interactively.
        #[arg(long)]
        password_stdin: bool,
    },
    /// List existing users.
    List,
    /// Change a user's role.
    SetRole { username: String, role: String },
    /// Reset a user's password.
    ResetPassword { username: String },
}

#[derive(Subcommand)]
pub enum DomainBlockCommand {
    /// Block a remote domain from federating with this instance.
    Add { domain: String },
    /// Remove a domain from the block list.
    Remove { domain: String },
}
