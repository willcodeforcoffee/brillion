pub mod actors;
pub mod users;

use sqlx::postgres::{PgPool, PgPoolOptions};

pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!().run(pool).await?;
    Ok(())
}
