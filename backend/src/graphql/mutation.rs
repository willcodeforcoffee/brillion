use super::types::{
    ActorGql, ActorImageKind, CreatePostInput, ImageUploadTarget, PostGql, UpdatePostInput,
};
use super::{parse_id, to_post_gql, Viewer};
use crate::db;
use crate::markdown;
use crate::media::MediaStore;
use async_graphql::{Context, Object, ID};
use sqlx::PgPool;
use uuid::Uuid;

pub struct Mutation;

#[Object]
impl Mutation {
    async fn create_post(
        &self,
        ctx: &Context<'_>,
        input: CreatePostInput,
    ) -> async_graphql::Result<PostGql> {
        let actor_id = ctx.data::<Viewer>()?.require_actor()?;
        let pool = ctx.data::<PgPool>()?;

        let body_html = markdown::render(&input.body_markdown);
        let post = db::posts::create(
            pool,
            db::posts::NewPost {
                actor_id,
                title: &input.title,
                summary: input.summary.as_deref(),
                body_markdown: &input.body_markdown,
                body_html: &body_html,
            },
        )
        .await?;

        Ok(to_post_gql(pool, post).await?)
    }

    async fn update_post(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdatePostInput,
    ) -> async_graphql::Result<PostGql> {
        let actor_id = ctx.data::<Viewer>()?.require_actor()?;
        let pool = ctx.data::<PgPool>()?;
        let post_id = parse_id(&id)?;
        own_post_or_error(pool, post_id, actor_id).await?;

        let body_html = markdown::render(&input.body_markdown);
        let post = db::posts::update(
            pool,
            post_id,
            db::posts::PostEdit {
                title: &input.title,
                summary: input.summary.as_deref(),
                body_markdown: &input.body_markdown,
                body_html: &body_html,
            },
        )
        .await?;

        Ok(to_post_gql(pool, post).await?)
    }

    async fn publish_post(&self, ctx: &Context<'_>, id: ID) -> async_graphql::Result<PostGql> {
        let actor_id = ctx.data::<Viewer>()?.require_actor()?;
        let pool = ctx.data::<PgPool>()?;
        let post_id = parse_id(&id)?;
        own_post_or_error(pool, post_id, actor_id).await?;

        let post = db::posts::publish(pool, post_id).await?;
        Ok(to_post_gql(pool, post).await?)
    }

    async fn delete_post(&self, ctx: &Context<'_>, id: ID) -> async_graphql::Result<bool> {
        let actor_id = ctx.data::<Viewer>()?.require_actor()?;
        let pool = ctx.data::<PgPool>()?;
        let post_id = parse_id(&id)?;
        own_post_or_error(pool, post_id, actor_id).await?;

        db::posts::delete(pool, post_id).await?;
        Ok(true)
    }

    /// Returns a presigned RustFS upload URL (SPEC.md §5.2) — the
    /// client `PUT`s the file bytes there directly, then calls
    /// `attachImage`/`updateActorImage` with the returned `mediaId`.
    async fn request_image_upload(
        &self,
        ctx: &Context<'_>,
        content_type: String,
    ) -> async_graphql::Result<ImageUploadTarget> {
        let actor_id = ctx.data::<Viewer>()?.require_actor()?;
        let pool = ctx.data::<PgPool>()?;
        let media_store = ctx.data::<MediaStore>()?;

        let extension = extension_for_content_type(&content_type)?;
        let object_key = format!("uploads/{actor_id}/{}.{extension}", Uuid::new_v4());
        let (upload_url, public_url) = media_store.presign_upload(&object_key, &content_type);

        let media = db::media_attachments::create(
            pool,
            db::media_attachments::NewMediaAttachment {
                object_key: &object_key,
                url: &public_url,
                media_type: &content_type,
                byte_size: 0,
                alt_text: None,
            },
        )
        .await?;

        Ok(ImageUploadTarget {
            upload_url,
            media_id: ID(media.id.to_string()),
            public_url,
        })
    }

    async fn attach_image(
        &self,
        ctx: &Context<'_>,
        post_id: ID,
        media_id: ID,
        alt_text: Option<String>,
    ) -> async_graphql::Result<PostGql> {
        let actor_id = ctx.data::<Viewer>()?.require_actor()?;
        let pool = ctx.data::<PgPool>()?;
        let post_id = parse_id(&post_id)?;
        let media_id = parse_id(&media_id)?;
        own_post_or_error(pool, post_id, actor_id).await?;

        db::media_attachments::attach_to_post(pool, media_id, post_id).await?;
        if let Some(alt_text) = alt_text {
            sqlx::query("update media_attachments set alt_text = $1 where id = $2")
                .bind(alt_text)
                .bind(media_id)
                .execute(pool)
                .await?;
        }

        let post = db::posts::find_by_id(pool, post_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("post not found"))?;
        Ok(to_post_gql(pool, post).await?)
    }

    async fn update_actor_image(
        &self,
        ctx: &Context<'_>,
        kind: ActorImageKind,
        media_id: ID,
    ) -> async_graphql::Result<ActorGql> {
        let actor_id = ctx.data::<Viewer>()?.require_actor()?;
        let pool = ctx.data::<PgPool>()?;
        let media_id = parse_id(&media_id)?;

        let media = db::media_attachments::find_by_id(pool, media_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("media not found"))?;

        match kind {
            ActorImageKind::Avatar => {
                db::actors::update_avatar_url(pool, actor_id, &media.url).await?
            }
            ActorImageKind::Header => {
                db::actors::update_header_url(pool, actor_id, &media.url).await?
            }
        }

        let actor = db::actors::find_by_id(pool, actor_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("actor not found"))?;
        Ok(actor.into())
    }
}

async fn own_post_or_error(
    pool: &PgPool,
    post_id: Uuid,
    actor_id: Uuid,
) -> async_graphql::Result<()> {
    let post = db::posts::find_by_id(pool, post_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("post not found"))?;
    if post.actor_id != actor_id {
        return Err(async_graphql::Error::new("not your post"));
    }
    Ok(())
}

fn extension_for_content_type(content_type: &str) -> async_graphql::Result<&'static str> {
    match content_type {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/webp" => Ok("webp"),
        "image/gif" => Ok("gif"),
        other => Err(async_graphql::Error::new(format!(
            "unsupported image content type: {other}"
        ))),
    }
}
