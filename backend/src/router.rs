use crate::routes::{actor, collections, inbox, nodeinfo, webfinger};
use crate::state::AppState;
use axum::routing::{get, post};
use axum::Router;

/// Builds the application router. Kept separate from `main` so it can be
/// exercised directly in tests without binding a real socket.
///
/// TODO(later phases): mount `/graphql` (§6), public blog pages and
/// `/feed.{rss,atom}` (§4), and the `/oauth/*` + `/api/v1/apps`
/// endpoints (§7).
pub fn app(state: AppState) -> Router {
    let public = Router::new().route("/health", get(health));

    let federation = Router::new()
        .route("/.well-known/webfinger", get(webfinger::handler))
        .route("/.well-known/nodeinfo", get(nodeinfo::well_known))
        .route("/nodeinfo/2.1", get(nodeinfo::handler))
        .route("/users/:username", get(actor::handler))
        .route("/users/:username/inbox", post(inbox::handler))
        .route("/inbox", post(inbox::handler))
        .route("/users/:username/followers", get(collections::followers))
        .route("/users/:username/following", get(collections::following))
        .route("/users/:username/outbox", get(collections::outbox))
        .with_state(state);

    public.merge(federation)
}

async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        // `connect_lazy` doesn't touch the network — fine for routes
        // (like /health) that never actually query the database.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://user:pass@localhost/brillion_test")
            .expect("lazy pool construction should not fail");
        AppState::new(
            pool,
            Config {
                database_url: String::new(),
                public_base_url: "http://localhost:3000".to_string(),
                port: 3000,
            },
        )
    }

    #[tokio::test]
    async fn health_check_returns_ok() {
        let response = app(test_state())
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"ok");
    }
}
