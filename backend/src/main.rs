mod cli;
mod router;

use clap::Parser;
use cli::{Cli, Command, DomainBlockCommand, UserCommand};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match Cli::parse().command {
        Command::Serve => serve().await,
        Command::Migrate => {
            println!("brillion migrate: not yet implemented — see SPEC.md §9 (Data model) / sqlx migrate");
            Ok(())
        }
        Command::User { command } => user_command(command),
        Command::DomainBlock { command } => domain_block_command(command),
    }
}

async fn serve() -> anyhow::Result<()> {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "brillion listening");
    axum::serve(listener, router::app()).await?;
    Ok(())
}

fn user_command(command: UserCommand) -> anyhow::Result<()> {
    match command {
        UserCommand::Create {
            email,
            username,
            role,
            password_stdin: _,
        } => println!(
            "brillion user create: not yet implemented (email={email}, username={username}, role={role}) — see SPEC.md §8 (CLI) / §9 (Data model)"
        ),
        UserCommand::List => {
            println!("brillion user list: not yet implemented — see SPEC.md §8")
        }
        UserCommand::SetRole { username, role } => println!(
            "brillion user set-role: not yet implemented ({username} -> {role})"
        ),
        UserCommand::ResetPassword { username } => println!(
            "brillion user reset-password: not yet implemented ({username})"
        ),
    }
    Ok(())
}

fn domain_block_command(command: DomainBlockCommand) -> anyhow::Result<()> {
    match command {
        DomainBlockCommand::Add { domain } => {
            println!("brillion domain-block add: not yet implemented ({domain})")
        }
        DomainBlockCommand::Remove { domain } => {
            println!("brillion domain-block remove: not yet implemented ({domain})")
        }
    }
    Ok(())
}
