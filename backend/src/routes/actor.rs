//! `GET /users/:username` — content-negotiated actor document (AS2
//! `Person`) or a placeholder HTML page. SPEC.md §3.2 / §4.1.

use crate::routes::as2_json;
use crate::state::AppState;
use activitypub::object::Person;
use activitypub::urls::ActorUrls;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};

pub async fn handler(
    State(state): State<AppState>,
    Path(username): Path<String>,
    headers: HeaderMap,
) -> Response {
    let actor = match crate::db::actors::find_local_by_username(&state.pool, &username).await {
        Ok(Some(actor)) => actor,
        Ok(None) => return (StatusCode::NOT_FOUND, "no such user").into_response(),
        Err(error) => {
            tracing::error!(%error, "actor lookup failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let wants_activitypub = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| accept.contains("activity+json") || accept.contains("ld+json"));

    if wants_activitypub {
        let urls = ActorUrls::new(&state.config.public_base_url, &actor.preferred_username);
        let person = Person::new(
            &urls,
            &actor.preferred_username,
            &actor.display_name,
            &actor.bio,
            &actor.public_key_pem,
            actor.avatar_url.as_deref(),
            actor.header_url.as_deref(),
        );
        as2_json(person)
    } else {
        // Public profile pages are the rest of phase 1 (SPEC.md §4.1),
        // not built yet — a plain placeholder keeps the route resolvable.
        Html(format!(
            "<!doctype html><title>@{username}</title><p>@{username} — public profile page isn't built yet.</p>"
        ))
        .into_response()
    }
}
