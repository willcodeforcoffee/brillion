use crate::config::Config;
use crate::graphql::{self, BrillionSchema};
use crate::media::MediaStore;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub http: reqwest::Client,
    pub media: MediaStore,
    pub graphql_schema: BrillionSchema,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder().build()?;
        let media = MediaStore::new(
            &config.s3_endpoint_internal,
            &config.public_base_url,
            config.s3_bucket.clone(),
            config.s3_access_key.clone(),
            config.s3_secret_key.clone(),
        )?;
        let graphql_schema = graphql::build_schema(pool.clone(), media.clone());
        Ok(Self {
            pool,
            config,
            http,
            media,
            graphql_schema,
        })
    }
}
