//! `GET /.well-known/webfinger` — SPEC.md §3.1.

use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct WebfingerQuery {
    resource: String,
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<WebfingerQuery>,
) -> Response {
    let Some(acct) = query.resource.strip_prefix("acct:") else {
        return (StatusCode::BAD_REQUEST, "resource must be an acct: URI").into_response();
    };
    let Some((username, domain)) = acct.split_once('@') else {
        return (StatusCode::BAD_REQUEST, "resource must be acct:user@domain").into_response();
    };
    if domain != state.config.domain() {
        return (StatusCode::NOT_FOUND, "unknown domain").into_response();
    }

    match crate::db::actors::find_local_by_username(&state.pool, username).await {
        Ok(Some(actor)) => {
            let body = json!({
                "subject": query.resource,
                "links": [{
                    "rel": "self",
                    "type": "application/activity+json",
                    "href": actor.ap_id,
                }],
            });
            (
                StatusCode::OK,
                [("Content-Type", "application/jrd+json")],
                Json(body),
            )
                .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "no such user").into_response(),
        Err(error) => {
            tracing::error!(%error, "webfinger lookup failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}
