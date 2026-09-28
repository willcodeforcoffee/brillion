//! View models for the server-rendered public pages (SPEC.md §4.1,
//! §11/§12 #2) — homepage, per-author page, post permalink. Kept
//! separate from `routes/` since the same summary shape is shared by
//! more than one page.

use crate::db;
use sqlx::PgPool;

pub struct PostSummary {
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub author_username: String,
    pub published_at: String,
}

/// Loads each post's author to build the view model — no
/// DataLoader/batching, same tradeoff as `graphql::to_post_gql`.
pub async fn post_summaries(
    pool: &PgPool,
    posts: Vec<db::posts::Post>,
) -> anyhow::Result<Vec<PostSummary>> {
    let mut summaries = Vec::with_capacity(posts.len());
    for post in posts {
        let author_username = db::actors::find_by_id(pool, post.actor_id)
            .await?
            .map(|actor| actor.preferred_username)
            .unwrap_or_else(|| "unknown".to_string());
        summaries.push(PostSummary {
            slug: post.slug,
            title: post.title,
            summary: post.summary,
            author_username,
            published_at: post
                .published_at
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        });
    }
    Ok(summaries)
}
