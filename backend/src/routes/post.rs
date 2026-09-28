//! `GET /users/:username/:slug` — a single post: content-negotiated
//! between the AS2 `Article` object (§3.5) and a server-rendered
//! permalink page (§4.1, §11/§12 #2).

use crate::routes::as2_json;
use crate::state::AppState;
use activitypub::object::Article;
use askama::Template;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate {
    title: String,
    summary: Option<String>,
    author_username: String,
    published_at: String,
    body_html: String,
}

pub async fn handler(
    State(state): State<AppState>,
    Path((username, slug)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let post =
        match crate::db::posts::find_published_by_actor_and_slug(&state.pool, &username, &slug)
            .await
        {
            Ok(Some(post)) => post,
            Ok(None) => return (StatusCode::NOT_FOUND, "no such post").into_response(),
            Err(error) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
        };

    let wants_activitypub = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| accept.contains("activity+json") || accept.contains("ld+json"));

    let published_at = post
        .published_at
        .map(|t| t.to_rfc3339())
        .unwrap_or_default();

    if wants_activitypub {
        let author = match crate::db::actors::find_by_id(&state.pool, post.actor_id).await {
            Ok(Some(actor)) => actor,
            Ok(None) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "post has no author").into_response()
            }
            Err(error) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
        };
        let permalink = format!("{}/users/{username}/{slug}", state.config.public_base_url);
        let article = Article::new(
            &permalink,
            &post.title,
            post.summary.as_deref(),
            &post.body_html,
            &author.ap_id,
            &permalink,
            Some(&published_at),
        );
        as2_json(article)
    } else {
        let template = PostTemplate {
            title: post.title,
            summary: post.summary,
            author_username: username,
            published_at,
            body_html: post.body_html,
        };
        match template.render() {
            Ok(html) => Html(html).into_response(),
            Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
        }
    }
}
