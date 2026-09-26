use sqlx::PgExecutor;
use uuid::Uuid;

/// Logs an inbound activity, for audit/debugging and idempotency.
/// Returns `false` (and logs nothing further) if `ap_id` was already
/// seen — inbox handlers should skip re-processing in that case, per
/// SPEC.md §3.3 ("dedupes by activity id").
pub async fn record_inbound<'e, E>(
    executor: E,
    ap_id: &str,
    activity_json: &serde_json::Value,
    actor_id: Option<Uuid>,
) -> anyhow::Result<bool>
where
    E: PgExecutor<'e>,
{
    let result = sqlx::query(
        "insert into activities_log (direction, ap_id, activity_json, actor_id, processed_at)
         values ('in', $1, $2, $3, now())
         on conflict (ap_id) do nothing",
    )
    .bind(ap_id)
    .bind(activity_json)
    .bind(actor_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}
