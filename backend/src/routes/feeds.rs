//! `GET /feed.{rss,atom}` and `GET /users/:username/feed.{rss,atom}` —
//! SPEC.md §4.2. Full post content (§12 #6, decided), built from the
//! same `posts` query that backs the homepage/author pages (§4.1) so
//! the views can't drift apart.

use crate::db;
use crate::state::AppState;
use atom_syndication::{
    ContentBuilder, Entry as AtomEntry, EntryBuilder as AtomEntryBuilder, Feed,
    FeedBuilder as AtomFeedBuilder, Link as AtomLink, LinkBuilder as AtomLinkBuilder,
    PersonBuilder as AtomPersonBuilder,
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use rss::{Channel, ChannelBuilder, GuidBuilder, Item, ItemBuilder};

const FEED_LIMIT: i64 = 20;

struct FeedEntry {
    title: String,
    permalink: String,
    author_username: String,
    summary: Option<String>,
    body_html: String,
    published_rfc2822: String,
    published_rfc3339: String,
}

async fn load_entries(
    state: &AppState,
    actor_id: Option<uuid::Uuid>,
) -> anyhow::Result<Vec<FeedEntry>> {
    let posts = db::posts::list_published(&state.pool, actor_id, None, FEED_LIMIT).await?;
    let mut entries = Vec::with_capacity(posts.len());
    for post in posts {
        let author = db::actors::find_by_id(&state.pool, post.actor_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("post {} has no author actor", post.id))?;
        let published = post.published_at.unwrap_or(post.created_at);
        entries.push(FeedEntry {
            title: post.title,
            permalink: format!(
                "{}/users/{}/{}",
                state.config.public_base_url, author.preferred_username, post.slug
            ),
            author_username: author.preferred_username,
            summary: post.summary,
            body_html: post.body_html,
            published_rfc2822: published.to_rfc2822(),
            published_rfc3339: published.to_rfc3339(),
        });
    }
    Ok(entries)
}

fn rss_channel(title: &str, link: &str, entries: Vec<FeedEntry>) -> Channel {
    let items: Vec<Item> = entries
        .into_iter()
        .map(|entry| {
            ItemBuilder::default()
                .title(Some(entry.title))
                .link(Some(entry.permalink.clone()))
                .guid(Some(
                    GuidBuilder::default()
                        .value(entry.permalink)
                        .permalink(true)
                        .build(),
                ))
                .author(Some(entry.author_username))
                .description(entry.summary)
                .content(Some(entry.body_html))
                .pub_date(Some(entry.published_rfc2822))
                .build()
        })
        .collect();

    ChannelBuilder::default()
        .title(title)
        .link(link)
        .description(format!("{title} — recent posts"))
        .items(items)
        .build()
}

fn atom_feed(title: &str, link: &str, entries: Vec<FeedEntry>) -> Feed {
    let updated = entries
        .first()
        .map(|e| e.published_rfc3339.clone())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let atom_entries: Vec<AtomEntry> = entries
        .into_iter()
        .map(|entry| {
            AtomEntryBuilder::default()
                .title(entry.title)
                .id(entry.permalink.clone())
                .updated(
                    chrono::DateTime::parse_from_rfc3339(&entry.published_rfc3339)
                        .unwrap_or_else(|_| chrono::Utc::now().into()),
                )
                .published(chrono::DateTime::parse_from_rfc3339(&entry.published_rfc3339).ok())
                .authors(vec![AtomPersonBuilder::default()
                    .name(entry.author_username)
                    .build()])
                .summary(entry.summary.map(Into::into))
                .content(Some(
                    ContentBuilder::default()
                        .value(Some(entry.body_html))
                        .content_type(Some("html".to_string()))
                        .build(),
                ))
                .links(vec![AtomLinkBuilder::default()
                    .href(entry.permalink)
                    .build()])
                .build()
        })
        .collect();

    AtomFeedBuilder::default()
        .title(title)
        .id(link)
        .updated(
            chrono::DateTime::parse_from_rfc3339(&updated)
                .unwrap_or_else(|_| chrono::Utc::now().into()),
        )
        .links(vec![
            AtomLinkBuilder::default().href(link).build() as AtomLink
        ])
        .entries(atom_entries)
        .build()
}

fn rss_response(channel: Channel) -> Response {
    (
        StatusCode::OK,
        [("Content-Type", "application/rss+xml")],
        channel.to_string(),
    )
        .into_response()
}

fn atom_response(feed: Feed) -> Response {
    (
        StatusCode::OK,
        [("Content-Type", "application/atom+xml")],
        feed.to_string(),
    )
        .into_response()
}

pub async fn site_rss(State(state): State<AppState>) -> Response {
    match load_entries(&state, None).await {
        Ok(entries) => rss_response(rss_channel(
            "Brillion",
            &state.config.public_base_url,
            entries,
        )),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn site_atom(State(state): State<AppState>) -> Response {
    match load_entries(&state, None).await {
        Ok(entries) => atom_response(atom_feed(
            "Brillion",
            &state.config.public_base_url,
            entries,
        )),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn find_actor_or_404(
    state: &AppState,
    username: &str,
) -> Result<db::actors::Actor, (StatusCode, String)> {
    match db::actors::find_local_by_username(&state.pool, username).await {
        Ok(Some(actor)) => Ok(actor),
        Ok(None) => Err((StatusCode::NOT_FOUND, "no such user".to_string())),
        Err(error) => Err((StatusCode::INTERNAL_SERVER_ERROR, error.to_string())),
    }
}

pub async fn actor_rss(State(state): State<AppState>, Path(username): Path<String>) -> Response {
    let actor = match find_actor_or_404(&state, &username).await {
        Ok(actor) => actor,
        Err(response) => return response.into_response(),
    };
    match load_entries(&state, Some(actor.id)).await {
        Ok(entries) => rss_response(rss_channel(
            &format!("@{username}"),
            &format!("{}/users/{username}", state.config.public_base_url),
            entries,
        )),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn actor_atom(State(state): State<AppState>, Path(username): Path<String>) -> Response {
    let actor = match find_actor_or_404(&state, &username).await {
        Ok(actor) => actor,
        Err(response) => return response.into_response(),
    };
    match load_entries(&state, Some(actor.id)).await {
        Ok(entries) => atom_response(atom_feed(
            &format!("@{username}"),
            &format!("{}/users/{username}", state.config.public_base_url),
            entries,
        )),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}
