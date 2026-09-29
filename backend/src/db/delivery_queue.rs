//! Outbound federation delivery queue (SPEC.md §3.4/§9) — rows are
//! enqueued by `crate::delivery`'s `enqueue_*` helpers when a post is
//! published/edited/deleted, and drained by `crate::delivery::run_worker`.

use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

/// Terminal after this many failed attempts — the row stays around
/// (`status = 'failed'`) for debugging via `last_error`, but is never
/// retried again.
const MAX_ATTEMPTS: i32 = 5;

/// Backoff before each retry, indexed by `attempts` after incrementing
/// (so index 0 is the delay before the *first* retry, after the initial
/// attempt already failed once): 1m, 5m, 30m, 2h, then terminal.
const BACKOFF_SECS: [i64; MAX_ATTEMPTS as usize - 1] = [60, 300, 1800, 7200];

#[derive(Debug, sqlx::FromRow)]
pub struct QueuedDelivery {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub inbox_url: String,
    pub activity_json: Value,
    pub attempts: i32,
}

pub async fn enqueue<'e, E>(
    executor: E,
    actor_id: Uuid,
    inbox_url: &str,
    activity_json: &Value,
) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query(
        "insert into delivery_queue (actor_id, inbox_url, activity_json) values ($1, $2, $3)",
    )
    .bind(actor_id)
    .bind(inbox_url)
    .bind(activity_json)
    .execute(executor)
    .await?;
    Ok(())
}

/// Atomically claims up to `limit` due rows by pushing their
/// `next_attempt_at` forward as a short lease (`FOR UPDATE SKIP LOCKED`
/// so multiple worker instances, if ever run, don't double-claim) —
/// see the note on the `delivery_queue` migration for why there's no
/// separate 'in_progress' status.
pub async fn claim_due<'e, E>(executor: E, limit: i64) -> anyhow::Result<Vec<QueuedDelivery>>
where
    E: PgExecutor<'e>,
{
    let rows = sqlx::query_as::<_, QueuedDelivery>(
        "update delivery_queue set next_attempt_at = now() + interval '5 minutes', updated_at = now()
         where id in (
             select id from delivery_queue
             where status = 'pending' and next_attempt_at <= now()
             order by next_attempt_at
             limit $1
             for update skip locked
         )
         returning id, actor_id, inbox_url, activity_json, attempts",
    )
    .bind(limit)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn mark_delivered<'e, E>(executor: E, id: Uuid) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query("update delivery_queue set status = 'delivered', updated_at = now() where id = $1")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Records a failed delivery attempt: schedules a backoff retry, or
/// marks the row permanently `failed` once `MAX_ATTEMPTS` is reached.
pub async fn mark_failed<'e, E>(
    executor: E,
    id: Uuid,
    attempts_before: i32,
    error: &str,
) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    let attempts = attempts_before + 1;
    if attempts >= MAX_ATTEMPTS {
        sqlx::query(
            "update delivery_queue set status = 'failed', attempts = $2, last_error = $3,
             updated_at = now() where id = $1",
        )
        .bind(id)
        .bind(attempts)
        .bind(error)
        .execute(executor)
        .await?;
    } else {
        let backoff_secs = BACKOFF_SECS[(attempts - 1) as usize];
        sqlx::query(
            "update delivery_queue set status = 'pending', attempts = $2, last_error = $3,
             next_attempt_at = now() + make_interval(secs => $4), updated_at = now()
             where id = $1",
        )
        .bind(id)
        .bind(attempts)
        .bind(error)
        .bind(backoff_secs as f64)
        .execute(executor)
        .await?;
    }
    Ok(())
}
