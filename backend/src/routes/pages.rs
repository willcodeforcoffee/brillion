//! `GET /` — the public, unauthenticated homepage (SPEC.md §4.1). Not
//! part of the React SPA (§11/§12 #2, decided) — plain server-rendered
//! HTML via `askama`.

use crate::pages::PostSummary;
use crate::state::AppState;
use askama::Template;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
    posts: Vec<PostSummary>,
}

#[derive(Deserialize)]
pub struct HomepageQuery {
    before: Option<Uuid>,
}

pub async fn homepage(
    State(state): State<AppState>,
    Query(query): Query<HomepageQuery>,
) -> Response {
    let posts = match crate::db::posts::list_published(&state.pool, None, query.before, 20).await {
        Ok(posts) => posts,
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
        }
    };
    let summaries = match crate::pages::post_summaries(&state.pool, posts).await {
        Ok(summaries) => summaries,
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
        }
    };

    match (HomepageTemplate { posts: summaries }).render() {
        Ok(html) => Html(html).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}
