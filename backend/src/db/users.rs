use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum Role {
    Admin,
    Author,
}

impl std::str::FromStr for Role {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(Role::Admin),
            "author" => Ok(Role::Author),
            other => Err(anyhow::anyhow!(
                "invalid role '{other}', expected 'admin' or 'author'"
            )),
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Author => write!(f, "author"),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[allow(dead_code)] // read back for completeness; not used until login (phase 1 OAuth slice)
    pub password_hash: String,
    pub role: Role,
    #[allow(dead_code)] // ditto
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserSummary {
    pub username: String,
    pub email: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

pub async fn create<'e, E>(
    executor: E,
    email: &str,
    password_hash: &str,
    role: Role,
) -> anyhow::Result<User>
where
    E: PgExecutor<'e>,
{
    let user = sqlx::query_as::<_, User>(
        "insert into users (email, password_hash, role)
         values ($1, $2, $3)
         returning id, email, password_hash, role, created_at",
    )
    .bind(email)
    .bind(password_hash)
    .bind(role)
    .fetch_one(executor)
    .await?;
    Ok(user)
}

pub async fn list<'e, E>(executor: E) -> anyhow::Result<Vec<UserSummary>>
where
    E: PgExecutor<'e>,
{
    let rows = sqlx::query_as::<_, UserSummary>(
        "select a.preferred_username as username, u.email, u.role, u.created_at
         from users u
         join actors a on a.user_id = u.id
         where a.is_local = true
         order by u.created_at",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn find_by_username<'e, E>(executor: E, username: &str) -> anyhow::Result<Option<User>>
where
    E: PgExecutor<'e>,
{
    let user = sqlx::query_as::<_, User>(
        "select u.id, u.email, u.password_hash, u.role, u.created_at
         from users u
         join actors a on a.user_id = u.id
         where a.preferred_username = $1 and a.is_local = true",
    )
    .bind(username)
    .fetch_optional(executor)
    .await?;
    Ok(user)
}

pub async fn set_role<'e, E>(executor: E, user_id: Uuid, role: Role) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query("update users set role = $1 where id = $2")
        .bind(role)
        .bind(user_id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn set_password_hash<'e, E>(
    executor: E,
    user_id: Uuid,
    password_hash: &str,
) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query("update users set password_hash = $1 where id = $2")
        .bind(password_hash)
        .bind(user_id)
        .execute(executor)
        .await?;
    Ok(())
}
