use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Post {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub body_markdown: String,
    pub body_html: String,
    pub status: String,
    #[allow(dead_code)] // populated once outbound federation (phase 3) exists
    pub ap_object_id: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Post {
    pub fn is_published(&self) -> bool {
        self.status == "published"
    }
}

pub struct NewPost<'a> {
    pub actor_id: Uuid,
    pub title: &'a str,
    pub summary: Option<&'a str>,
    pub body_markdown: &'a str,
    pub body_html: &'a str,
}

/// Inserts a draft post, generating a unique-per-actor slug from the
/// title (retrying with a numeric suffix on collision).
pub async fn create<'e, E>(executor: E, new: NewPost<'_>) -> anyhow::Result<Post>
where
    E: PgExecutor<'e> + Copy,
{
    let base_slug = slugify(new.title);

    for attempt in 0..50 {
        let slug = if attempt == 0 {
            base_slug.clone()
        } else {
            format!("{base_slug}-{}", attempt + 1)
        };

        let result = sqlx::query_as::<_, Post>(
            "insert into posts (actor_id, slug, title, summary, body_markdown, body_html)
             values ($1, $2, $3, $4, $5, $6)
             returning id, actor_id, slug, title, summary, body_markdown, body_html,
                 status, ap_object_id, published_at, created_at, updated_at",
        )
        .bind(new.actor_id)
        .bind(&slug)
        .bind(new.title)
        .bind(new.summary)
        .bind(new.body_markdown)
        .bind(new.body_html)
        .fetch_one(executor)
        .await;

        match result {
            Ok(post) => return Ok(post),
            Err(sqlx::Error::Database(db_err))
                if db_err.constraint() == Some("posts_actor_id_slug_key") =>
            {
                continue;
            }
            Err(err) => return Err(err.into()),
        }
    }

    anyhow::bail!("could not find a unique slug for title {:?}", new.title)
}

pub struct PostEdit<'a> {
    pub title: &'a str,
    pub summary: Option<&'a str>,
    pub body_markdown: &'a str,
    pub body_html: &'a str,
}

pub async fn update<'e, E>(executor: E, post_id: Uuid, edit: PostEdit<'_>) -> anyhow::Result<Post>
where
    E: PgExecutor<'e>,
{
    let post = sqlx::query_as::<_, Post>(
        "update posts set title = $1, summary = $2, body_markdown = $3, body_html = $4,
             updated_at = now()
         where id = $5
         returning id, actor_id, slug, title, summary, body_markdown, body_html,
             status, ap_object_id, published_at, created_at, updated_at",
    )
    .bind(edit.title)
    .bind(edit.summary)
    .bind(edit.body_markdown)
    .bind(edit.body_html)
    .bind(post_id)
    .fetch_one(executor)
    .await?;
    Ok(post)
}

pub async fn publish<'e, E>(executor: E, post_id: Uuid) -> anyhow::Result<Post>
where
    E: PgExecutor<'e>,
{
    let post = sqlx::query_as::<_, Post>(
        "update posts set status = 'published', published_at = coalesce(published_at, now()),
             updated_at = now()
         where id = $1
         returning id, actor_id, slug, title, summary, body_markdown, body_html,
             status, ap_object_id, published_at, created_at, updated_at",
    )
    .bind(post_id)
    .fetch_one(executor)
    .await?;
    Ok(post)
}

pub async fn delete<'e, E>(executor: E, post_id: Uuid) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query("delete from posts where id = $1")
        .bind(post_id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn find_by_id<'e, E>(executor: E, post_id: Uuid) -> anyhow::Result<Option<Post>>
where
    E: PgExecutor<'e>,
{
    let post = sqlx::query_as::<_, Post>(
        "select id, actor_id, slug, title, summary, body_markdown, body_html,
             status, ap_object_id, published_at, created_at, updated_at
         from posts where id = $1",
    )
    .bind(post_id)
    .fetch_optional(executor)
    .await?;
    Ok(post)
}

pub async fn find_published_by_actor_and_slug<'e, E>(
    executor: E,
    preferred_username: &str,
    slug: &str,
) -> anyhow::Result<Option<Post>>
where
    E: PgExecutor<'e>,
{
    let post = sqlx::query_as::<_, Post>(
        "select p.id, p.actor_id, p.slug, p.title, p.summary, p.body_markdown, p.body_html,
             p.status, p.ap_object_id, p.published_at, p.created_at, p.updated_at
         from posts p
         join actors a on a.id = p.actor_id
         where a.preferred_username = $1 and a.is_local = true
           and p.slug = $2 and p.status = 'published'",
    )
    .bind(preferred_username)
    .bind(slug)
    .fetch_optional(executor)
    .await?;
    Ok(post)
}

/// Newest-first, published posts — site-wide (`actor_id = None`) or
/// scoped to one author. Simple keyset pagination: `after` is the id of
/// the last post already seen, excluded via a `published_at`/`id`
/// tiebreak so pagination is stable even with equal timestamps.
pub async fn list_published<'e, E>(
    executor: E,
    actor_id: Option<Uuid>,
    after: Option<Uuid>,
    limit: i64,
) -> anyhow::Result<Vec<Post>>
where
    E: PgExecutor<'e>,
{
    let posts = sqlx::query_as::<_, Post>(
        "select id, actor_id, slug, title, summary, body_markdown, body_html,
             status, ap_object_id, published_at, created_at, updated_at
         from posts
         where status = 'published'
           and ($1::uuid is null or actor_id = $1)
           and (
             $2::uuid is null
             or (published_at, id) < (
               select published_at, id from posts where id = $2
             )
           )
         order by published_at desc, id desc
         limit $3",
    )
    .bind(actor_id)
    .bind(after)
    .bind(limit)
    .fetch_all(executor)
    .await?;
    Ok(posts)
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_hyphen = true;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            slug.push('-');
            last_was_hyphen = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "post".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_punctuation_and_case() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  leading/trailing  "), "leading-trailing");
        assert_eq!(slugify("日本語"), "post");
        assert_eq!(slugify(""), "post");
    }
}
