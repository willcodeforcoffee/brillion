//! OAuth 2.0 provider storage — SPEC.md §7. Tokens/codes/secrets are
//! never stored in plaintext: `crypto::generate_token` produces the
//! value returned to the caller once, `crypto::hash_token` produces
//! what's actually stored, and lookups re-hash the presented value.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Application {
    pub id: Uuid,
    #[allow(dead_code)]
    pub name: String,
    pub client_id: String,
    pub client_secret_hash: String,
    pub redirect_uris: Vec<String>,
    pub scopes: String,
}

pub struct NewApplication<'a> {
    pub name: &'a str,
    pub redirect_uris: &'a [String],
    pub scopes: &'a str,
}

pub struct CreatedApplication {
    pub application: Application,
    /// Plaintext — shown to the caller exactly once, never stored.
    pub client_secret: String,
}

pub async fn create_application<'e, E>(
    executor: E,
    new: NewApplication<'_>,
) -> anyhow::Result<CreatedApplication>
where
    E: PgExecutor<'e>,
{
    let client_id = crate::crypto::generate_token();
    let client_secret = crate::crypto::generate_token();
    let client_secret_hash = crate::crypto::hash_token(&client_secret);

    let application = sqlx::query_as::<_, Application>(
        "insert into oauth_applications (name, client_id, client_secret_hash, redirect_uris, scopes)
         values ($1, $2, $3, $4, $5)
         returning id, name, client_id, client_secret_hash, redirect_uris, scopes",
    )
    .bind(new.name)
    .bind(&client_id)
    .bind(&client_secret_hash)
    .bind(new.redirect_uris)
    .bind(new.scopes)
    .fetch_one(executor)
    .await?;

    Ok(CreatedApplication {
        application,
        client_secret,
    })
}

pub async fn find_application_by_client_id<'e, E>(
    executor: E,
    client_id: &str,
) -> anyhow::Result<Option<Application>>
where
    E: PgExecutor<'e>,
{
    let application = sqlx::query_as::<_, Application>(
        "select id, name, client_id, client_secret_hash, redirect_uris, scopes
         from oauth_applications where client_id = $1",
    )
    .bind(client_id)
    .fetch_optional(executor)
    .await?;
    Ok(application)
}

pub struct NewAuthorizationCode<'a> {
    pub application_id: Uuid,
    pub user_id: Uuid,
    pub code_challenge: &'a str,
    pub redirect_uri: &'a str,
    pub scopes: &'a str,
}

/// Creates an authorization code (10-minute expiry) and returns its
/// plaintext value, to be delivered to the client via redirect.
pub async fn create_authorization_code<'e, E>(
    executor: E,
    new: NewAuthorizationCode<'_>,
) -> anyhow::Result<String>
where
    E: PgExecutor<'e>,
{
    let code = crate::crypto::generate_token();
    let code_hash = crate::crypto::hash_token(&code);
    let expires_at = Utc::now() + Duration::minutes(10);

    sqlx::query(
        "insert into oauth_authorization_codes
            (application_id, user_id, code_hash, code_challenge, redirect_uri, scopes, expires_at)
         values ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(new.application_id)
    .bind(new.user_id)
    .bind(&code_hash)
    .bind(new.code_challenge)
    .bind(new.redirect_uri)
    .bind(new.scopes)
    .bind(expires_at)
    .execute(executor)
    .await?;

    Ok(code)
}

#[derive(Debug, sqlx::FromRow)]
pub struct AuthorizationCode {
    pub application_id: Uuid,
    pub user_id: Uuid,
    pub code_challenge: String,
    pub redirect_uri: String,
    pub scopes: String,
    #[allow(dead_code)]
    pub expires_at: DateTime<Utc>,
}

/// Looks up a code by its plaintext value and atomically marks it used
/// — a code is valid for exactly one exchange. Returns `None` if it
/// doesn't exist, was already used, or has expired.
pub async fn consume_authorization_code<'e, E>(
    executor: E,
    code: &str,
) -> anyhow::Result<Option<AuthorizationCode>>
where
    E: PgExecutor<'e>,
{
    let code_hash = crate::crypto::hash_token(code);
    let row = sqlx::query_as::<_, AuthorizationCode>(
        "update oauth_authorization_codes
         set used = true
         where code_hash = $1 and used = false and expires_at > now()
         returning application_id, user_id, code_challenge, redirect_uri, scopes, expires_at",
    )
    .bind(&code_hash)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub struct IssuedToken {
    pub access_token: String,
    pub refresh_token: String,
    pub scopes: String,
}

pub async fn issue_token<'e, E>(
    executor: E,
    application_id: Uuid,
    user_id: Uuid,
    scopes: &str,
) -> anyhow::Result<IssuedToken>
where
    E: PgExecutor<'e>,
{
    let access_token = crate::crypto::generate_token();
    let refresh_token = crate::crypto::generate_token();
    let access_hash = crate::crypto::hash_token(&access_token);
    let refresh_hash = crate::crypto::hash_token(&refresh_token);

    sqlx::query(
        "insert into oauth_tokens (application_id, user_id, token_hash, refresh_token_hash, scopes)
         values ($1, $2, $3, $4, $5)",
    )
    .bind(application_id)
    .bind(user_id)
    .bind(&access_hash)
    .bind(&refresh_hash)
    .bind(scopes)
    .execute(executor)
    .await?;

    Ok(IssuedToken {
        access_token,
        refresh_token,
        scopes: scopes.to_string(),
    })
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Token {
    #[allow(dead_code)]
    pub application_id: Uuid,
    pub user_id: Uuid,
    #[allow(dead_code)]
    pub scopes: String,
}

/// Resolves a bearer access token to the user it belongs to — the
/// GraphQL auth context's job (SPEC.md §6).
pub async fn find_by_access_token<'e, E>(
    executor: E,
    access_token: &str,
) -> anyhow::Result<Option<Token>>
where
    E: PgExecutor<'e>,
{
    let hash = crate::crypto::hash_token(access_token);
    let token = sqlx::query_as::<_, Token>(
        "select application_id, user_id, scopes from oauth_tokens
         where token_hash = $1 and revoked_at is null",
    )
    .bind(&hash)
    .fetch_optional(executor)
    .await?;
    Ok(token)
}

/// Rotates a refresh token: revokes the old pair and issues a new one.
/// Returns `None` if the refresh token is unknown or already revoked.
pub async fn refresh<'e, E>(executor: E, refresh_token: &str) -> anyhow::Result<Option<IssuedToken>>
where
    E: PgExecutor<'e> + Copy,
{
    let hash = crate::crypto::hash_token(refresh_token);
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "select application_id, user_id, scopes from oauth_tokens
         where refresh_token_hash = $1 and revoked_at is null",
    )
    .bind(&hash)
    .fetch_optional(executor)
    .await?;

    let Some((application_id, user_id, scopes)) = row else {
        return Ok(None);
    };

    sqlx::query("update oauth_tokens set revoked_at = now() where refresh_token_hash = $1")
        .bind(&hash)
        .execute(executor)
        .await?;

    Ok(Some(
        issue_token(executor, application_id, user_id, &scopes).await?,
    ))
}

pub async fn revoke<'e, E>(executor: E, access_token: &str) -> anyhow::Result<()>
where
    E: PgExecutor<'e>,
{
    let hash = crate::crypto::hash_token(access_token);
    sqlx::query("update oauth_tokens set revoked_at = now() where token_hash = $1")
        .bind(&hash)
        .execute(executor)
        .await?;
    Ok(())
}
