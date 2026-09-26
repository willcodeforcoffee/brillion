use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)] // read back for completeness; not yet surfaced anywhere
pub struct Follow {
    pub id: Uuid,
    pub follower_actor_id: Uuid,
    pub followee_actor_id: Uuid,
    pub state: String,
    pub ap_activity_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Records an accepted follow (phase 2: every inbound `Follow` is
/// auto-accepted, per SPEC.md §3.3 — there's no approval workflow yet).
/// Idempotent: re-following just refreshes the activity id.
pub async fn upsert_accepted<'e, E>(
    executor: E,
    follower_actor_id: Uuid,
    followee_actor_id: Uuid,
    ap_activity_id: &str,
) -> anyhow::Result<Follow>
where
    E: PgExecutor<'e>,
{
    let follow = sqlx::query_as::<_, Follow>(
        "insert into follows (follower_actor_id, followee_actor_id, state, ap_activity_id)
         values ($1, $2, 'accepted', $3)
         on conflict (follower_actor_id, followee_actor_id)
         do update set state = 'accepted', ap_activity_id = excluded.ap_activity_id
         returning id, follower_actor_id, followee_actor_id, state, ap_activity_id, created_at",
    )
    .bind(follower_actor_id)
    .bind(followee_actor_id)
    .bind(ap_activity_id)
    .fetch_one(executor)
    .await?;
    Ok(follow)
}

/// Removes a follow relationship (an inbound `Undo` of a `Follow`).
pub async fn remove<'e, E>(
    executor: E,
    follower_actor_id: Uuid,
    followee_actor_id: Uuid,
) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query("delete from follows where follower_actor_id = $1 and followee_actor_id = $2")
        .bind(follower_actor_id)
        .bind(followee_actor_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// AS2 `id` URLs of actors following `followee_actor_id` — for the
/// `followers` collection (SPEC.md §3.3).
pub async fn follower_ap_ids<'e, E>(
    executor: E,
    followee_actor_id: Uuid,
) -> anyhow::Result<Vec<String>>
where
    E: PgExecutor<'e>,
{
    let ap_ids: Vec<(String,)> = sqlx::query_as(
        "select a.ap_id from follows f
         join actors a on a.id = f.follower_actor_id
         where f.followee_actor_id = $1 and f.state = 'accepted'
         order by f.created_at",
    )
    .bind(followee_actor_id)
    .fetch_all(executor)
    .await?;
    Ok(ap_ids.into_iter().map(|(id,)| id).collect())
}

/// AS2 `id` URLs of actors `follower_actor_id` is following — for the
/// `following` collection (SPEC.md §3.3).
pub async fn following_ap_ids<'e, E>(
    executor: E,
    follower_actor_id: Uuid,
) -> anyhow::Result<Vec<String>>
where
    E: PgExecutor<'e>,
{
    let ap_ids: Vec<(String,)> = sqlx::query_as(
        "select a.ap_id from follows f
         join actors a on a.id = f.followee_actor_id
         where f.follower_actor_id = $1 and f.state = 'accepted'
         order by f.created_at",
    )
    .bind(follower_actor_id)
    .fetch_all(executor)
    .await?;
    Ok(ap_ids.into_iter().map(|(id,)| id).collect())
}
