use crate::config::Config;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let http = reqwest::Client::builder()
            .build()
            .expect("failed to build HTTP client");
        Self { pool, config, http }
    }
}
