mod cli;
mod config;
mod crypto;
mod db;
mod router;

use activitypub::urls::ActorUrls;
use clap::Parser;
use cli::{Cli, Command, DomainBlockCommand, UserCommand};
use config::Config;
use db::users::Role;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::str::FromStr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match Cli::parse().command {
        Command::Serve => {
            let config = Config::from_env()?;
            let pool = db::connect(&config.database_url).await?;
            db::run_migrations(&pool).await?;
            serve(&config).await
        }
        Command::Migrate => {
            let config = Config::from_env()?;
            let pool = db::connect(&config.database_url).await?;
            db::run_migrations(&pool).await?;
            println!("Migrations applied.");
            Ok(())
        }
        Command::User { command } => {
            let config = Config::from_env()?;
            let pool = db::connect(&config.database_url).await?;
            user_command(&pool, &config, command).await
        }
        Command::DomainBlock { command } => domain_block_command(command),
    }
}

async fn serve(config: &Config) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "brillion listening");
    axum::serve(listener, router::app()).await?;
    Ok(())
}

async fn user_command(pool: &PgPool, config: &Config, command: UserCommand) -> anyhow::Result<()> {
    match command {
        UserCommand::Create {
            email,
            username,
            role,
            password_stdin,
        } => {
            let role = Role::from_str(&role)?;
            let password = read_new_password(password_stdin)?;
            let password_hash = crypto::hash_password(&password)?;
            let keypair = activitypub::signature::generate_keypair()?;
            let domain = config.domain();
            let urls = ActorUrls::new(&config.public_base_url, &username);

            let mut tx = pool.begin().await?;
            let user = db::users::create(&mut *tx, &email, &password_hash, role).await?;
            let actor = db::actors::create_local(
                &mut *tx,
                db::actors::NewLocalActor {
                    user_id: user.id,
                    preferred_username: &username,
                    domain: &domain,
                    urls,
                    public_key_pem: keypair.public_key_pem,
                    private_key_pem: keypair.private_key_pem,
                },
            )
            .await?;
            tx.commit().await?;

            println!(
                "Created user {email} (@{username}@{domain}, role={role}) — actor {}",
                actor.id
            );
        }
        UserCommand::List => {
            let users = db::users::list(pool).await?;
            if users.is_empty() {
                println!("No users.");
            } else {
                for u in users {
                    println!(
                        "{}\t{}\t{}\t{}",
                        u.username,
                        u.email,
                        u.role,
                        u.created_at.to_rfc3339()
                    );
                }
            }
        }
        UserCommand::SetRole { username, role } => {
            let role = Role::from_str(&role)?;
            let user = db::users::find_by_username(pool, &username)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no such user: {username}"))?;
            db::users::set_role(pool, user.id, role).await?;
            println!(
                "Set {username}'s ({}) role: {} -> {role}.",
                user.email, user.role
            );
        }
        UserCommand::ResetPassword { username } => {
            let user = db::users::find_by_username(pool, &username)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no such user: {username}"))?;
            let password = read_new_password(false)?;
            let password_hash = crypto::hash_password(&password)?;
            db::users::set_password_hash(pool, user.id, &password_hash).await?;
            println!("Password reset for {username} ({}).", user.email);
        }
    }
    Ok(())
}

/// Reads a new password either from stdin (one line, for scripted use)
/// or via a hidden interactive prompt with confirmation.
fn read_new_password(from_stdin: bool) -> anyhow::Result<String> {
    if from_stdin {
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf)?;
        Ok(buf.trim_end_matches(['\r', '\n']).to_string())
    } else {
        let password = rpassword::prompt_password("Password: ")?;
        let confirmation = rpassword::prompt_password("Confirm password: ")?;
        anyhow::ensure!(password == confirmation, "passwords do not match");
        Ok(password)
    }
}

fn domain_block_command(command: DomainBlockCommand) -> anyhow::Result<()> {
    match command {
        DomainBlockCommand::Add { domain } => println!(
            "brillion domain-block add: not yet implemented ({domain}) — federation lands in phase 2, SPEC.md §3"
        ),
        DomainBlockCommand::Remove { domain } => println!(
            "brillion domain-block remove: not yet implemented ({domain}) — federation lands in phase 2, SPEC.md §3"
        ),
    }
    Ok(())
}
