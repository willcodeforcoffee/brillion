//! Outbound federation delivery (SPEC.md §3.4) — builds `Create`/
//! `Update`/`Delete` activities for post publish/edit/delete, enqueues
//! one `delivery_queue` row per follower inbox, and a background worker
//! (spawned from `main.rs`'s `serve()`) drains the queue with
//! retry/backoff. No separate `worker` process (§10, §12: in-process).
//!
//! **Known simplification, not an oversight:** delivery only reaches an
//! author's followers. SPEC.md §3.4 also calls for delivering to
//! actors `@mentioned` in the post body — that needs an outbound
//! WebFinger client to resolve arbitrary `@user@domain` handles to an
//! actor, which doesn't exist yet anywhere in this codebase (only the
//! inbound WebFinger *responder*, `routes::webfinger`, does). Left for
//! a follow-up rather than half-implemented here.

use crate::db;
use crate::db::actors::Actor;
use crate::db::posts::Post;
use crate::federation;
use crate::state::AppState;
use activitypub::activity::{Create, Delete, Update};
use activitypub::object::{Article, Tombstone};
use activitypub::AS2_PUBLIC;
use serde_json::Value;
use sqlx::PgPool;
use std::time::Duration;

/// An `Article` for `post`, serialized without its own `@context` —
/// nested contexts are legal JSON-LD but unnecessary noise once it's
/// embedded in a `Create`/`Update` activity that already has one.
fn article_json(post: &Post, author: &Actor, permalink: &str) -> anyhow::Result<Value> {
    let published = post.published_at.map(|t| t.to_rfc3339());
    let article = Article::new(
        permalink,
        &post.title,
        post.summary.as_deref(),
        &post.body_html,
        &author.ap_id,
        permalink,
        published.as_deref(),
    );
    let mut value = serde_json::to_value(article)?;
    value
        .as_object_mut()
        .expect("Article always serializes to a JSON object")
        .remove("@context");
    Ok(value)
}

/// Enqueues a `Create` to every accepted follower's inbox — call once,
/// the first time a post transitions draft → published (the caller
/// checks that; re-publishing an already-published post is a no-op
/// here, matching `db::posts::publish`'s `coalesce`d `ap_object_id`).
pub async fn enqueue_create(pool: &PgPool, author: &Actor, post: &Post) -> anyhow::Result<()> {
    let permalink = post
        .ap_object_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("post {} has no ap_object_id to federate", post.id))?;
    let object = article_json(post, author, permalink)?;
    let published = post
        .published_at
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let activity = Create::new(
        format!("{permalink}#create"),
        &author.ap_id,
        published,
        vec![AS2_PUBLIC.to_string()],
        vec![author.followers_url.clone()],
        object,
    );
    enqueue_to_followers(pool, author, &serde_json::to_value(activity)?).await
}

/// Enqueues an `Update` — call when an already-published post is
/// edited (the caller checks that it was published *before* the edit).
pub async fn enqueue_update(pool: &PgPool, author: &Actor, post: &Post) -> anyhow::Result<()> {
    let permalink = post
        .ap_object_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("post {} has no ap_object_id to federate", post.id))?;
    let object = article_json(post, author, permalink)?;
    let activity = Update::new(
        format!("{permalink}#update-{}", post.updated_at.timestamp()),
        &author.ap_id,
        post.updated_at.to_rfc3339(),
        vec![AS2_PUBLIC.to_string()],
        vec![author.followers_url.clone()],
        object,
    );
    enqueue_to_followers(pool, author, &serde_json::to_value(activity)?).await
}

/// Enqueues a `Delete` wrapping a `Tombstone` — call when an
/// already-published post is deleted, *before* the `posts` row itself
/// is gone (needs `permalink`, which lived on that row).
pub async fn enqueue_delete(pool: &PgPool, author: &Actor, permalink: &str) -> anyhow::Result<()> {
    let tombstone = serde_json::to_value(Tombstone::new(permalink))?;
    let now = chrono::Utc::now();
    let activity = Delete::new(
        format!("{permalink}#delete-{}", now.timestamp()),
        &author.ap_id,
        now.to_rfc3339(),
        vec![AS2_PUBLIC.to_string()],
        vec![author.followers_url.clone()],
        tombstone,
    );
    enqueue_to_followers(pool, author, &serde_json::to_value(activity)?).await
}

async fn enqueue_to_followers(
    pool: &PgPool,
    author: &Actor,
    activity_json: &Value,
) -> anyhow::Result<()> {
    let inbox_urls = db::follows::follower_inbox_urls(pool, author.id).await?;
    for inbox_url in inbox_urls {
        db::delivery_queue::enqueue(pool, author.id, &inbox_url, activity_json).await?;
    }
    Ok(())
}

/// Runs forever, polling `delivery_queue` for due rows — spawned as a
/// background Tokio task from `main.rs`'s `serve()`.
pub async fn run_worker(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        if let Err(error) = process_due(&state).await {
            tracing::error!(%error, "delivery worker tick failed");
        }
    }
}

/// Drains one batch of due rows. Public (not `pub(crate)`) so the
/// integration test can drive it directly instead of waiting on
/// `run_worker`'s 10-second tick — see the same reasoning on
/// `Config::test_default` for why this needs to be a plain `pub` item
/// rather than `#[cfg(test)]`-gated (integration tests under
/// `backend/tests/` link this crate as an ordinary dependency).
pub async fn process_due(state: &AppState) -> anyhow::Result<()> {
    let batch = db::delivery_queue::claim_due(&state.pool, 20).await?;
    for item in batch {
        let Some(actor) = db::actors::find_by_id(&state.pool, item.actor_id).await? else {
            tracing::error!(actor_id = %item.actor_id, "delivery queue references unknown actor");
            continue;
        };
        let Some(private_key_pem) = actor.private_key_pem.as_deref() else {
            tracing::error!(actor_id = %actor.id, "delivery queue actor has no private key");
            continue;
        };
        let key_id = format!("{}#main-key", actor.ap_id);

        match federation::deliver(
            &state.http,
            private_key_pem,
            &key_id,
            &item.inbox_url,
            &item.activity_json,
        )
        .await
        {
            Ok(()) => db::delivery_queue::mark_delivered(&state.pool, item.id).await?,
            Err(error) => {
                tracing::warn!(%error, inbox = %item.inbox_url, attempts = item.attempts, "delivery attempt failed");
                db::delivery_queue::mark_failed(
                    &state.pool,
                    item.id,
                    item.attempts,
                    &error.to_string(),
                )
                .await?;
            }
        }
    }
    Ok(())
}
