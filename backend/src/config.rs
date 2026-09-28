use anyhow::Context;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub public_base_url: String,
    pub port: u16,
    /// Origin the React dev server runs on (Vite's default) — allowed
    /// via CORS to call `/graphql`/`/oauth/*`/`/api/v1/apps` across
    /// origins in development. In production the frontend is served
    /// from the same origin as the backend, so this only matters there
    /// if that changes.
    pub frontend_origin: String,
    pub s3_endpoint_internal: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let public_base_url = std::env::var("PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        let s3_endpoint_internal = std::env::var("S3_ENDPOINT_INTERNAL")
            .unwrap_or_else(|_| "http://localhost:9000".to_string());
        let s3_bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "brillion-media".to_string());
        // Not `.context(...)?`-required like DATABASE_URL: most CLI
        // subcommands (`user *`, `migrate`, `domain-block *`) never
        // touch RustFS at all, and Config::from_env() runs for all of
        // them. A missing/wrong credential only matters once `serve`
        // actually tries to sign a request against RustFS.
        let s3_access_key =
            std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "brillion".to_string());
        let s3_secret_key =
            std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "changeme".to_string());
        let frontend_origin = std::env::var("FRONTEND_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:5173".to_string());

        Ok(Self {
            database_url,
            public_base_url,
            port,
            frontend_origin,
            s3_endpoint_internal,
            s3_bucket,
            s3_access_key,
            s3_secret_key,
        })
    }

    /// The bare hostname this instance is reachable at, used as the
    /// `domain` for local actors (`user@domain` addressing — SPEC.md
    /// §3.1). Derived from `public_base_url` rather than configured
    /// separately, so the two can't drift out of sync.
    pub fn domain(&self) -> String {
        let without_scheme = self
            .public_base_url
            .split("://")
            .nth(1)
            .unwrap_or(&self.public_base_url);
        let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
        host_port.split(':').next().unwrap_or(host_port).to_string()
    }

    /// Convenience defaults for tests (unit tests in this crate and
    /// integration tests under `backend/tests/`, which link this crate
    /// as an ordinary dependency and so don't get `cfg(test)` items).
    pub fn test_default() -> Self {
        Self {
            database_url: String::new(),
            public_base_url: "http://localhost:3000".to_string(),
            port: 3000,
            frontend_origin: "http://localhost:5173".to_string(),
            s3_endpoint_internal: "http://localhost:9000".to_string(),
            s3_bucket: "test-bucket".to_string(),
            s3_access_key: "test-access-key".to_string(),
            s3_secret_key: "test-secret-key".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_strips_scheme_port_and_path() {
        let config = Config {
            public_base_url: "https://brillion.example.com:8443/".to_string(),
            ..Config::test_default()
        };
        assert_eq!(config.domain(), "brillion.example.com");
    }
}
