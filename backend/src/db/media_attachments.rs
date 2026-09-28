use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)] // most fields aren't read back until image display lands in templates/GraphQL
pub struct MediaAttachment {
    pub id: Uuid,
    pub post_id: Option<Uuid>,
    pub object_key: String,
    pub url: String,
    pub media_type: String,
    pub byte_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub alt_text: Option<String>,
}

pub struct NewMediaAttachment<'a> {
    pub object_key: &'a str,
    pub url: &'a str,
    pub media_type: &'a str,
    pub byte_size: i64,
    pub alt_text: Option<&'a str>,
}

/// Creates an as-yet-unattached media row (the object has been uploaded
/// to RustFS but not yet linked to a post) — see `attach_to_post`.
pub async fn create<'e, E>(
    executor: E,
    new: NewMediaAttachment<'_>,
) -> anyhow::Result<MediaAttachment>
where
    E: PgExecutor<'e>,
{
    let media = sqlx::query_as::<_, MediaAttachment>(
        "insert into media_attachments (object_key, url, media_type, byte_size, alt_text)
         values ($1, $2, $3, $4, $5)
         returning id, post_id, object_key, url, media_type, byte_size, width, height, alt_text",
    )
    .bind(new.object_key)
    .bind(new.url)
    .bind(new.media_type)
    .bind(new.byte_size)
    .bind(new.alt_text)
    .fetch_one(executor)
    .await?;
    Ok(media)
}

pub async fn attach_to_post<'e, E>(
    executor: E,
    media_id: Uuid,
    post_id: Uuid,
) -> anyhow::Result<MediaAttachment>
where
    E: PgExecutor<'e>,
{
    let media = sqlx::query_as::<_, MediaAttachment>(
        "update media_attachments set post_id = $1 where id = $2
         returning id, post_id, object_key, url, media_type, byte_size, width, height, alt_text",
    )
    .bind(post_id)
    .bind(media_id)
    .fetch_one(executor)
    .await?;
    Ok(media)
}

pub async fn find_by_id<'e, E>(
    executor: E,
    media_id: Uuid,
) -> anyhow::Result<Option<MediaAttachment>>
where
    E: PgExecutor<'e>,
{
    let media = sqlx::query_as::<_, MediaAttachment>(
        "select id, post_id, object_key, url, media_type, byte_size, width, height, alt_text
         from media_attachments where id = $1",
    )
    .bind(media_id)
    .fetch_optional(executor)
    .await?;
    Ok(media)
}

pub async fn find_by_post<'e, E>(executor: E, post_id: Uuid) -> anyhow::Result<Vec<MediaAttachment>>
where
    E: PgExecutor<'e>,
{
    let media = sqlx::query_as::<_, MediaAttachment>(
        "select id, post_id, object_key, url, media_type, byte_size, width, height, alt_text
         from media_attachments where post_id = $1",
    )
    .bind(post_id)
    .fetch_all(executor)
    .await?;
    Ok(media)
}
