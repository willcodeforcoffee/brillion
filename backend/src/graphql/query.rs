use super::types::{ActorGql, PageInfo, PostConnection, PostEdge, UserGql};
use super::{to_post_gql, Viewer};
use crate::db;
use async_graphql::{Context, Object, ID};
use sqlx::PgPool;
use uuid::Uuid;

pub struct Query;

#[Object]
impl Query {
    /// The signed-in user's own actor, or `null` if anonymous.
    async fn me(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<ActorGql>> {
        let viewer = ctx.data::<Viewer>()?;
        let Some(actor_id) = viewer.actor_id else {
            return Ok(None);
        };
        let pool = ctx.data::<PgPool>()?;
        Ok(db::actors::find_by_id(pool, actor_id)
            .await?
            .map(Into::into))
    }

    /// A single published post by its author's username and slug.
    async fn post(
        &self,
        ctx: &Context<'_>,
        author: String,
        slug: String,
    ) -> async_graphql::Result<Option<super::types::PostGql>> {
        let pool = ctx.data::<PgPool>()?;
        let Some(post) = db::posts::find_published_by_actor_and_slug(pool, &author, &slug).await?
        else {
            return Ok(None);
        };
        Ok(Some(to_post_gql(pool, post).await?))
    }

    /// Published posts, newest first — site-wide, or scoped to one
    /// author via `authorId`.
    async fn posts(
        &self,
        ctx: &Context<'_>,
        author_id: Option<ID>,
        first: Option<i32>,
        after: Option<String>,
    ) -> async_graphql::Result<PostConnection> {
        let pool = ctx.data::<PgPool>()?;
        let actor_id = author_id.map(|id| super::parse_id(&id)).transpose()?;
        let after_id = after
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| async_graphql::Error::new("invalid cursor"))?;
        let limit = first.unwrap_or(20).clamp(1, 100) as i64;

        // Fetch one extra row to know whether there's a next page.
        let mut posts = db::posts::list_published(pool, actor_id, after_id, limit + 1).await?;
        let has_next_page = posts.len() as i64 > limit;
        posts.truncate(limit as usize);

        let end_cursor = posts.last().map(|p| p.id.to_string());
        let mut edges = Vec::with_capacity(posts.len());
        for post in posts {
            let cursor = post.id.to_string();
            edges.push(PostEdge {
                cursor,
                node: to_post_gql(pool, post).await?,
            });
        }

        Ok(PostConnection {
            edges,
            page_info: PageInfo {
                has_next_page,
                end_cursor,
            },
        })
    }

    /// A local actor by their preferred username.
    async fn actor(
        &self,
        ctx: &Context<'_>,
        handle: String,
    ) -> async_graphql::Result<Option<ActorGql>> {
        let pool = ctx.data::<PgPool>()?;
        Ok(db::actors::find_local_by_username(pool, &handle)
            .await?
            .map(Into::into))
    }

    /// A post by id, including unpublished drafts — for the editor.
    /// Only the post's own author or an admin may fetch it.
    async fn post_by_id(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> async_graphql::Result<Option<super::types::PostGql>> {
        let viewer = ctx.data::<Viewer>()?;
        let pool = ctx.data::<PgPool>()?;
        let post_id = super::parse_id(&id)?;

        let Some(post) = db::posts::find_by_id(pool, post_id).await? else {
            return Ok(None);
        };
        if viewer.actor_id != Some(post.actor_id) && viewer.require_admin().is_err() {
            return Err(async_graphql::Error::new("not authorized"));
        }
        Ok(Some(to_post_gql(pool, post).await?))
    }

    /// All local users — admin only.
    async fn users(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<UserGql>> {
        ctx.data::<Viewer>()?.require_admin()?;
        let pool = ctx.data::<PgPool>()?;
        Ok(db::users::list(pool)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }
}
