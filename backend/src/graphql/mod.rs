//! GraphQL API — SPEC.md §6. The client-facing surface for the React
//! app; distinct from the federation (§3) and public-page (§4) HTTP
//! surfaces, though all three read the same `posts`/`actors` tables.

mod mutation;
mod query;
pub mod types;

use crate::db;
use crate::media::MediaStore;
use async_graphql::{EmptySubscription, Schema};
use mutation::Mutation;
use query::Query;
use sqlx::PgPool;
use types::PostGql;
use uuid::Uuid;

pub type BrillionSchema = Schema<Query, Mutation, EmptySubscription>;

pub fn build_schema(pool: PgPool, media: MediaStore) -> BrillionSchema {
    Schema::build(Query, Mutation, EmptySubscription)
        .data(pool)
        .data(media)
        .finish()
}

/// Who's making the request, resolved from the `Authorization: Bearer`
/// header (see `routes::graphql`) — all `None` for anonymous, which
/// still gets read-only access to public data.
#[derive(Clone, Default)]
pub struct Viewer {
    pub user_id: Option<Uuid>,
    pub actor_id: Option<Uuid>,
    pub role: Option<db::users::Role>,
}

impl Viewer {
    pub fn require_actor(&self) -> async_graphql::Result<Uuid> {
        self.actor_id
            .ok_or_else(|| async_graphql::Error::new("authentication required"))
    }

    pub fn require_admin(&self) -> async_graphql::Result<()> {
        if self.role == Some(db::users::Role::Admin) {
            Ok(())
        } else {
            Err(async_graphql::Error::new("admin access required"))
        }
    }
}

pub(crate) fn parse_id(id: &async_graphql::ID) -> async_graphql::Result<Uuid> {
    Uuid::parse_str(id.as_str()).map_err(|_| async_graphql::Error::new("invalid id"))
}

/// Loads a post's author and attached images to build the full GraphQL
/// type — no DataLoader/batching yet (fine at single-blog scale).
pub(crate) async fn to_post_gql(pool: &PgPool, post: db::posts::Post) -> anyhow::Result<PostGql> {
    let author = db::actors::find_by_id(pool, post.actor_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("post {} has no author actor", post.id))?;
    let images = db::media_attachments::find_by_post(pool, post.id).await?;

    Ok(PostGql {
        id: async_graphql::ID(post.id.to_string()),
        slug: post.slug,
        title: post.title,
        summary: post.summary,
        body_markdown: post.body_markdown,
        body_html: post.body_html,
        status: post.status,
        published_at: post.published_at,
        author: author.into(),
        images: images.into_iter().map(Into::into).collect(),
    })
}
