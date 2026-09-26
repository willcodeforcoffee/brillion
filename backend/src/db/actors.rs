use activitypub::urls::ActorUrls;
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)] // most fields aren't read until the actor/outbox routes land (phase 2/3)
pub struct Actor {
    pub id: Uuid,
    /// The actor's own AS2 `id` URL — for local actors, always
    /// `{PUBLIC_BASE_URL}/users/{preferred_username}`; for remote actors,
    /// whatever they reported in their own Person document.
    pub ap_id: String,
    pub user_id: Option<Uuid>,
    pub preferred_username: String,
    pub domain: String,
    pub display_name: String,
    pub bio: String,
    pub avatar_url: Option<String>,
    pub header_url: Option<String>,
    pub inbox_url: String,
    pub outbox_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub public_key_pem: String,
    pub private_key_pem: Option<String>,
    pub is_local: bool,
    pub created_at: DateTime<Utc>,
}

pub struct NewLocalActor<'a> {
    pub user_id: Uuid,
    pub preferred_username: &'a str,
    pub domain: &'a str,
    pub urls: ActorUrls,
    pub public_key_pem: String,
    pub private_key_pem: String,
}

pub async fn create_local<'e, E>(executor: E, new: NewLocalActor<'_>) -> anyhow::Result<Actor>
where
    E: PgExecutor<'e>,
{
    let actor = sqlx::query_as::<_, Actor>(
        "insert into actors
            (ap_id, user_id, preferred_username, domain, inbox_url, outbox_url,
             followers_url, following_url, public_key_pem, private_key_pem, is_local)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, true)
         returning id, ap_id, user_id, preferred_username, domain, display_name, bio,
             avatar_url, header_url, inbox_url, outbox_url, followers_url,
             following_url, public_key_pem, private_key_pem, is_local, created_at",
    )
    .bind(&new.urls.id)
    .bind(new.user_id)
    .bind(new.preferred_username)
    .bind(new.domain)
    .bind(&new.urls.inbox)
    .bind(&new.urls.outbox)
    .bind(&new.urls.followers)
    .bind(&new.urls.following)
    .bind(&new.public_key_pem)
    .bind(&new.private_key_pem)
    .fetch_one(executor)
    .await?;
    Ok(actor)
}

/// A remote actor's details, as parsed from their own Person document —
/// fetched and cached the first time we see them (e.g. in an inbound
/// `Follow`'s `actor` field), per SPEC.md §3.3.
pub struct NewRemoteActor {
    pub ap_id: String,
    pub preferred_username: String,
    pub domain: String,
    pub display_name: String,
    pub inbox_url: String,
    pub outbox_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub public_key_pem: String,
}

/// Inserts a remote actor, or refreshes the cached copy if we already
/// know them (their key may have rotated, display name may have
/// changed, etc.).
///
/// NOTE: only guards against a conflicting `ap_id` — a *different*
/// remote actor somehow reusing the same `(preferred_username, domain)`
/// (e.g. an actor migration that changes their `id`) will surface as a
/// raw constraint-violation error rather than a handled case. Judged an
/// acceptable gap for phase 2; revisit if actor migration is ever
/// supported.
pub async fn upsert_remote<'e, E>(executor: E, new: NewRemoteActor) -> anyhow::Result<Actor>
where
    E: PgExecutor<'e>,
{
    let actor = sqlx::query_as::<_, Actor>(
        "insert into actors
            (ap_id, preferred_username, domain, display_name, inbox_url, outbox_url,
             followers_url, following_url, public_key_pem, is_local)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, false)
         on conflict (ap_id) do update set
             display_name = excluded.display_name,
             inbox_url = excluded.inbox_url,
             outbox_url = excluded.outbox_url,
             followers_url = excluded.followers_url,
             following_url = excluded.following_url,
             public_key_pem = excluded.public_key_pem
         returning id, ap_id, user_id, preferred_username, domain, display_name, bio,
             avatar_url, header_url, inbox_url, outbox_url, followers_url,
             following_url, public_key_pem, private_key_pem, is_local, created_at",
    )
    .bind(&new.ap_id)
    .bind(&new.preferred_username)
    .bind(&new.domain)
    .bind(&new.display_name)
    .bind(&new.inbox_url)
    .bind(&new.outbox_url)
    .bind(&new.followers_url)
    .bind(&new.following_url)
    .bind(&new.public_key_pem)
    .fetch_one(executor)
    .await?;
    Ok(actor)
}

pub async fn find_by_ap_id<'e, E>(executor: E, ap_id: &str) -> anyhow::Result<Option<Actor>>
where
    E: PgExecutor<'e>,
{
    let actor = sqlx::query_as::<_, Actor>(
        "select id, ap_id, user_id, preferred_username, domain, display_name, bio,
             avatar_url, header_url, inbox_url, outbox_url, followers_url,
             following_url, public_key_pem, private_key_pem, is_local, created_at
         from actors where ap_id = $1",
    )
    .bind(ap_id)
    .fetch_optional(executor)
    .await?;
    Ok(actor)
}

pub async fn find_local_by_username<'e, E>(
    executor: E,
    preferred_username: &str,
) -> anyhow::Result<Option<Actor>>
where
    E: PgExecutor<'e>,
{
    let actor = sqlx::query_as::<_, Actor>(
        "select id, ap_id, user_id, preferred_username, domain, display_name, bio,
             avatar_url, header_url, inbox_url, outbox_url, followers_url,
             following_url, public_key_pem, private_key_pem, is_local, created_at
         from actors where preferred_username = $1 and is_local = true",
    )
    .bind(preferred_username)
    .fetch_optional(executor)
    .await?;
    Ok(actor)
}

pub async fn count_local<'e, E>(executor: E) -> anyhow::Result<i64>
where
    E: PgExecutor<'e>,
{
    let (count,): (i64,) = sqlx::query_as("select count(*) from actors where is_local = true")
        .fetch_one(executor)
        .await?;
    Ok(count)
}
