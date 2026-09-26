use sqlx::PgExecutor;

pub async fn add<'e, E>(executor: E, domain: &str, reason: Option<&str>) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query(
        "insert into domain_blocks (domain, reason) values ($1, $2)
         on conflict (domain) do update set reason = excluded.reason",
    )
    .bind(domain)
    .bind(reason)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn remove<'e, E>(executor: E, domain: &str) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query("delete from domain_blocks where domain = $1")
        .bind(domain)
        .execute(executor)
        .await?;
    Ok(())
}

/// Whether `domain` (or its registrable form) is on the block list —
/// checked by the inbox before processing any activity (SPEC.md §3.3).
pub async fn is_blocked<'e, E>(executor: E, domain: &str) -> anyhow::Result<bool>
where
    E: PgExecutor<'e>,
{
    let row: Option<(i32,)> = sqlx::query_as("select 1 from domain_blocks where domain = $1")
        .bind(domain)
        .fetch_optional(executor)
        .await?;
    Ok(row.is_some())
}
