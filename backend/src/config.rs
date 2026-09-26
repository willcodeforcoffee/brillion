use anyhow::Context;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub public_base_url: String,
    pub port: u16,
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

        Ok(Self {
            database_url,
            public_base_url,
            port,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_strips_scheme_port_and_path() {
        let config = Config {
            database_url: String::new(),
            public_base_url: "https://brillion.example.com:8443/".to_string(),
            port: 3000,
        };
        assert_eq!(config.domain(), "brillion.example.com");
    }
}
