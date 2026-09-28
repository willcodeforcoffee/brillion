//! `GET /users/:username` — content-negotiated actor document (AS2
//! `Person`) or a server-rendered profile page. SPEC.md §3.2 / §4.1.

use crate::pages::PostSummary;
use crate::routes::as2_json;
use crate::state::AppState;
use activitypub::object::Person;
use activitypub::urls::ActorUrls;
use askama::Template;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};

#[derive(Template)]
#[template(path = "actor_profile.html")]
struct ActorProfileTemplate {
    username: String,
    bio: String,
    posts: Vec<PostSummary>,
}

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
        let posts =
            match crate::db::posts::list_published(&state.pool, Some(actor.id), None, 20).await {
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

        let template = ActorProfileTemplate {
            username: actor.preferred_username,
            bio: actor.bio,
            posts: summaries,
        };
        match template.render() {
            Ok(html) => Html(html).into_response(),
            Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
        }
    }
}
