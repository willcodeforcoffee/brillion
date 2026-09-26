//! `GET /users/:username/{followers,following,outbox}` — SPEC.md §3.3.
//!
//! Non-paginated (fine at this scale — see the note on
//! `activitypub::object::OrderedCollection`). `outbox` is an empty
//! collection until posts land (phase 3).

use crate::db::actors::Actor;
use crate::routes::as2_json;
use crate::state::AppState;
use activitypub::object::OrderedCollection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

async fn find_local_actor(
    state: &AppState,
    username: &str,
) -> Result<Actor, (StatusCode, &'static str)> {
    match crate::db::actors::find_local_by_username(&state.pool, username).await {
        Ok(Some(actor)) => Ok(actor),
        Ok(None) => Err((StatusCode::NOT_FOUND, "no such user")),
        Err(error) => {
            tracing::error!(%error, "actor lookup failed");
            Err((StatusCode::INTERNAL_SERVER_ERROR, "internal error"))
        }
    }
}

pub async fn followers(State(state): State<AppState>, Path(username): Path<String>) -> Response {
    let actor = match find_local_actor(&state, &username).await {
        Ok(actor) => actor,
        Err(response) => return response.into_response(),
    };
    match crate::db::follows::follower_ap_ids(&state.pool, actor.id).await {
        Ok(ap_ids) => as2_json(OrderedCollection::new(&actor.followers_url, ap_ids)),
        Err(error) => {
            tracing::error!(%error, "followers lookup failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

pub async fn following(State(state): State<AppState>, Path(username): Path<String>) -> Response {
    let actor = match find_local_actor(&state, &username).await {
        Ok(actor) => actor,
        Err(response) => return response.into_response(),
    };
    match crate::db::follows::following_ap_ids(&state.pool, actor.id).await {
        Ok(ap_ids) => as2_json(OrderedCollection::new(&actor.following_url, ap_ids)),
        Err(error) => {
            tracing::error!(%error, "following lookup failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

pub async fn outbox(State(state): State<AppState>, Path(username): Path<String>) -> Response {
    let actor = match find_local_actor(&state, &username).await {
        Ok(actor) => actor,
        Err(response) => return response.into_response(),
    };
    as2_json(OrderedCollection::new(&actor.outbox_url, vec![]))
}
