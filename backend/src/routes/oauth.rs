//! OAuth 2.0 provider — SPEC.md §7. Authorization code + PKCE only (no
//! `password` grant). Mirrors Mastodon's client-auth API shape
//! (`/api/v1/apps`, `/oauth/*`).

use crate::state::AppState;
use askama::Template;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{Form, Json};
use serde::{Deserialize, Serialize};

#[derive(Template)]
#[template(path = "oauth_login.html")]
struct OAuthLoginTemplate<'a> {
    app_name: &'a str,
    error: Option<&'a str>,
    client_id: &'a str,
    redirect_uri: &'a str,
    state: &'a str,
    code_challenge: &'a str,
    scope: &'a str,
}

fn render_login(template: OAuthLoginTemplate<'_>) -> Response {
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

// --- POST /api/v1/apps ---------------------------------------------------

#[derive(Deserialize)]
pub struct CreateAppRequest {
    name: String,
    redirect_uris: Vec<String>,
    #[serde(default = "default_scopes")]
    scopes: String,
}

fn default_scopes() -> String {
    "read write follow".to_string()
}

#[derive(Serialize)]
pub struct CreateAppResponse {
    client_id: String,
    client_secret: String,
    name: String,
    redirect_uris: Vec<String>,
    scopes: String,
}

pub async fn create_app(
    State(state): State<AppState>,
    Json(body): Json<CreateAppRequest>,
) -> Response {
    let result = crate::db::oauth::create_application(
        &state.pool,
        crate::db::oauth::NewApplication {
            name: &body.name,
            redirect_uris: &body.redirect_uris,
            scopes: &body.scopes,
        },
    )
    .await;

    match result {
        Ok(created) => Json(CreateAppResponse {
            client_id: created.application.client_id,
            client_secret: created.client_secret,
            name: body.name,
            redirect_uris: body.redirect_uris,
            scopes: body.scopes,
        })
        .into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

// --- GET/POST /oauth/authorize -------------------------------------------

#[derive(Deserialize)]
pub struct AuthorizeQuery {
    client_id: String,
    redirect_uri: String,
    #[serde(default)]
    code_challenge: String,
    #[serde(default)]
    code_challenge_method: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    scope: String,
}

pub async fn authorize_form(
    State(state): State<AppState>,
    Query(query): Query<AuthorizeQuery>,
) -> Response {
    if query.code_challenge_method != "S256" {
        return (
            StatusCode::BAD_REQUEST,
            "code_challenge_method must be S256",
        )
            .into_response();
    }

    let app = match crate::db::oauth::find_application_by_client_id(&state.pool, &query.client_id)
        .await
    {
        Ok(Some(app)) => app,
        Ok(None) => return (StatusCode::BAD_REQUEST, "unknown client_id").into_response(),
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
        }
    };
    if !app.redirect_uris.contains(&query.redirect_uri) {
        return (
            StatusCode::BAD_REQUEST,
            "redirect_uri not registered for this client",
        )
            .into_response();
    }

    render_login(OAuthLoginTemplate {
        app_name: &app.name,
        error: None,
        client_id: &query.client_id,
        redirect_uri: &query.redirect_uri,
        state: &query.state,
        code_challenge: &query.code_challenge,
        scope: &query.scope,
    })
}

#[derive(Deserialize)]
pub struct AuthorizeSubmission {
    client_id: String,
    redirect_uri: String,
    #[serde(default)]
    state: String,
    code_challenge: String,
    #[serde(default)]
    scope: String,
    email: String,
    password: String,
}

pub async fn authorize_submit(
    State(state): State<AppState>,
    Form(form): Form<AuthorizeSubmission>,
) -> Response {
    let app =
        match crate::db::oauth::find_application_by_client_id(&state.pool, &form.client_id).await {
            Ok(Some(app)) => app,
            Ok(None) => return (StatusCode::BAD_REQUEST, "unknown client_id").into_response(),
            Err(error) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
        };

    let reject = |error: &'static str| {
        render_login(OAuthLoginTemplate {
            app_name: &app.name,
            error: Some(error),
            client_id: &form.client_id,
            redirect_uri: &form.redirect_uri,
            state: &form.state,
            code_challenge: &form.code_challenge,
            scope: &form.scope,
        })
    };

    let user = match crate::db::users::find_by_email(&state.pool, &form.email).await {
        Ok(Some(user)) => user,
        Ok(None) => return reject("no account with that email"),
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
        }
    };
    match crate::crypto::verify_password(&form.password, &user.password_hash) {
        Ok(true) => {}
        _ => return reject("incorrect email or password"),
    }

    let code = match crate::db::oauth::create_authorization_code(
        &state.pool,
        crate::db::oauth::NewAuthorizationCode {
            application_id: app.id,
            user_id: user.id,
            code_challenge: &form.code_challenge,
            redirect_uri: &form.redirect_uri,
            scopes: &form.scope,
        },
    )
    .await
    {
        Ok(code) => code,
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
        }
    };

    let mut redirect_url = form.redirect_uri.clone();
    redirect_url.push(if redirect_url.contains('?') { '&' } else { '?' });
    redirect_url.push_str("code=");
    redirect_url
        .push_str(&url::form_urlencoded::byte_serialize(code.as_bytes()).collect::<String>());
    redirect_url.push_str("&state=");
    redirect_url
        .push_str(&url::form_urlencoded::byte_serialize(form.state.as_bytes()).collect::<String>());

    Redirect::to(&redirect_url).into_response()
}

// --- POST /oauth/token ----------------------------------------------------

#[derive(Deserialize)]
pub struct TokenForm {
    grant_type: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    code_verifier: String,
    #[serde(default)]
    redirect_uri: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: String,
    #[serde(default)]
    refresh_token: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: &'static str,
    scope: String,
}

pub async fn token(State(state): State<AppState>, Form(form): Form<TokenForm>) -> Response {
    match form.grant_type.as_str() {
        "authorization_code" => authorization_code_grant(&state, form).await,
        "refresh_token" => refresh_token_grant(&state, form).await,
        other => (
            StatusCode::BAD_REQUEST,
            format!("unsupported grant_type: {other}"),
        )
            .into_response(),
    }
}

async fn authorization_code_grant(state: &AppState, form: TokenForm) -> Response {
    let app =
        match crate::db::oauth::find_application_by_client_id(&state.pool, &form.client_id).await {
            Ok(Some(app)) => app,
            Ok(None) => return (StatusCode::UNAUTHORIZED, "unknown client").into_response(),
            Err(error) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
        };
    if crate::crypto::hash_token(&form.client_secret) != app.client_secret_hash {
        return (StatusCode::UNAUTHORIZED, "invalid client credentials").into_response();
    }

    let auth_code = match crate::db::oauth::consume_authorization_code(&state.pool, &form.code)
        .await
    {
        Ok(Some(code)) => code,
        Ok(None) => return (StatusCode::BAD_REQUEST, "invalid or expired code").into_response(),
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
        }
    };
    if auth_code.application_id != app.id || auth_code.redirect_uri != form.redirect_uri {
        return (
            StatusCode::BAD_REQUEST,
            "code does not match client_id/redirect_uri",
        )
            .into_response();
    }

    // PKCE (RFC 7636 S256): code_challenge == BASE64URL(SHA256(code_verifier)),
    // which is exactly what `hash_token` computes.
    if crate::crypto::hash_token(&form.code_verifier) != auth_code.code_challenge {
        return (StatusCode::BAD_REQUEST, "invalid code_verifier").into_response();
    }

    match crate::db::oauth::issue_token(&state.pool, app.id, auth_code.user_id, &auth_code.scopes)
        .await
    {
        Ok(issued) => Json(TokenResponse {
            access_token: issued.access_token,
            refresh_token: issued.refresh_token,
            token_type: "Bearer",
            scope: issued.scopes,
        })
        .into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn refresh_token_grant(state: &AppState, form: TokenForm) -> Response {
    match crate::db::oauth::refresh(&state.pool, &form.refresh_token).await {
        Ok(Some(issued)) => Json(TokenResponse {
            access_token: issued.access_token,
            refresh_token: issued.refresh_token,
            token_type: "Bearer",
            scope: issued.scopes,
        })
        .into_response(),
        Ok(None) => (StatusCode::BAD_REQUEST, "invalid refresh_token").into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

// --- POST /oauth/revoke ----------------------------------------------------

#[derive(Deserialize)]
pub struct RevokeForm {
    token: String,
}

pub async fn revoke(State(state): State<AppState>, Form(form): Form<RevokeForm>) -> Response {
    match crate::db::oauth::revoke(&state.pool, &form.token).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}
