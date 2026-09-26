use activitypub::urls::ActorUrls;
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)] // most fields aren't read until the actor/outbox routes land (phase 2/3)
pub struct Actor {
    pub id: Uuid,
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
            (user_id, preferred_username, domain, inbox_url, outbox_url,
             followers_url, following_url, public_key_pem, private_key_pem, is_local)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, true)
         returning id, user_id, preferred_username, domain, display_name, bio,
             avatar_url, header_url, inbox_url, outbox_url, followers_url,
             following_url, public_key_pem, private_key_pem, is_local, created_at",
    )
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
