//! `POST /graphql` (+ `GET /graphql` for GraphiQL) — SPEC.md §6.

use crate::db;
use crate::graphql::Viewer;
use crate::state::AppState;
use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};

pub async fn playground() -> impl IntoResponse {
    Html(GraphiQLSource::build().endpoint("/graphql").finish())
}

pub async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: GraphQLRequest,
) -> Response {
    let viewer = resolve_viewer(&state, &headers).await;
    let response = state
        .graphql_schema
        .execute(request.into_inner().data(viewer))
        .await;
    GraphQLResponse::from(response).into_response()
}

/// Resolves the `Authorization: Bearer <token>` header (if any) to a
/// `Viewer`. Anonymous requests still get read-only access to public
/// queries — only mutations require `Viewer::require_actor`.
async fn resolve_viewer(state: &AppState, headers: &HeaderMap) -> Viewer {
    let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return Viewer::default();
    };

    let Ok(Some(oauth_token)) = db::oauth::find_by_access_token(&state.pool, token).await else {
        return Viewer::default();
    };

    let actor = db::actors::find_local_by_user_id(&state.pool, oauth_token.user_id)
        .await
        .ok()
        .flatten();
    let role = db::users::find_by_id(&state.pool, oauth_token.user_id)
        .await
        .ok()
        .flatten()
        .map(|user| user.role);

    Viewer {
        user_id: Some(oauth_token.user_id),
        actor_id: actor.map(|a| a.id),
        role,
    }
}
