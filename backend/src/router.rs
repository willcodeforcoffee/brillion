use crate::routes::{
    actor, collections, feeds, graphql, inbox, nodeinfo, oauth, pages, post as post_page, webfinger,
};
use crate::state::AppState;
use axum::http::{header, Method};
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::GovernorLayer;
use tower_http::cors::CorsLayer;

/// Builds the application router. Kept separate from `main` so it can be
/// exercised directly in tests without binding a real socket.
pub fn app(state: AppState) -> Router {
    let public = Router::new().route("/health", get(health));

    // The React dev server (Vite) runs on a different origin than the
    // backend, so `/graphql`/`/oauth/*`/`/api/v1/apps` need CORS to be
    // callable from it. Auth is a Bearer token (never a cookie), so no
    // `allow_credentials` is needed.
    let cors = CorsLayer::new()
        .allow_origin(
            state
                .config
                .frontend_origin
                .parse::<axum::http::HeaderValue>()
                .expect("FRONTEND_ORIGIN must be a valid header value"),
        )
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    // Rate limiting on the inbox (SPEC.md §3.6) — blunts abuse from a
    // misbehaving/malicious remote server. `SmartIpKeyExtractor` reads
    // `X-Forwarded-For`/`Forwarded` first (falling back to the peer
    // address), since in production this sits behind the Caddy proxy
    // (§10) — the plain peer-IP extractor would otherwise rate-limit
    // by the proxy's own IP for every remote sender at once.
    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(SmartIpKeyExtractor)
            .per_second(2)
            .burst_size(20)
            .finish()
            .expect("static governor config is valid"),
    );
    let inbox_routes = Router::new()
        .route("/users/{username}/inbox", post(inbox::handler))
        .route("/inbox", post(inbox::handler))
        .layer(GovernorLayer::new(governor_config))
        .with_state(state.clone());

    let stateful = Router::new()
        .route("/", get(pages::homepage))
        .route("/feed.rss", get(feeds::site_rss))
        .route("/feed.atom", get(feeds::site_atom))
        .route("/.well-known/webfinger", get(webfinger::handler))
        .route("/.well-known/nodeinfo", get(nodeinfo::well_known))
        .route("/nodeinfo/2.1", get(nodeinfo::handler))
        .route("/users/{username}", get(actor::handler))
        .route("/users/{username}/followers", get(collections::followers))
        .route("/users/{username}/following", get(collections::following))
        .route("/users/{username}/outbox", get(collections::outbox))
        .route("/users/{username}/feed.rss", get(feeds::actor_rss))
        .route("/users/{username}/feed.atom", get(feeds::actor_atom))
        .route("/users/{username}/{slug}", get(post_page::handler))
        .route("/graphql", post(graphql::handler).get(graphql::playground))
        .route("/api/v1/apps", post(oauth::create_app))
        .route(
            "/oauth/authorize",
            get(oauth::authorize_form).post(oauth::authorize_submit),
        )
        .route("/oauth/token", post(oauth::token))
        .route("/oauth/revoke", post(oauth::revoke))
        .layer(cors)
        .with_state(state);

    public.merge(stateful).merge(inbox_routes)
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
        AppState::new(pool, Config::test_default()).expect("test AppState should construct")
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
