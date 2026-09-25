use axum::{routing::get, Router};

/// Builds the application router. Kept separate from `main` so it can be
/// exercised directly in tests without binding a real socket.
///
/// TODO(later phases): mount `/graphql` (§6), ActivityPub discovery/actor/
/// inbox/outbox routes (§3), public blog pages and `/feed.{rss,atom}`
/// (§4), and the `/oauth/*` + `/api/v1/apps` endpoints (§7).
pub fn app() -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_check_returns_ok() {
        let response = app()
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
