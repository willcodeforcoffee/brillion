//! `GET /.well-known/nodeinfo` + `GET /nodeinfo/2.1` — SPEC.md §3.1.

use crate::state::AppState;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

pub async fn well_known(State(state): State<AppState>) -> Response {
    let href = format!(
        "{}/nodeinfo/2.1",
        state.config.public_base_url.trim_end_matches('/')
    );
    Json(json!({
        "links": [{
            "rel": "http://nodeinfo.diaspora.software/ns/schema/2.1",
            "href": href,
        }],
    }))
    .into_response()
}

pub async fn handler(State(state): State<AppState>) -> Response {
    let total_users = crate::db::actors::count_local(&state.pool)
        .await
        .unwrap_or(0);

    Json(json!({
        "version": "2.1",
        "software": {
            "name": "brillion",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "protocols": ["activitypub"],
        "services": {"outbound": [], "inbound": []},
        "usage": {
            "users": {"total": total_users},
            "localPosts": 0,
        },
        "openRegistrations": false,
        "metadata": {},
    }))
    .into_response()
}
