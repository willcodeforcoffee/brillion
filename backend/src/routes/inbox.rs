//! `POST /users/:username/inbox` and `POST /inbox` (shared) — SPEC.md
//! §3.3. Both routes share this handler: the activity's own `object`
//! (for `Follow`) determines the target local actor, not the path, so
//! there's no meaningful difference between them yet (posts/mentions,
//! which do need per-actor addressing, land in phase 3).

use crate::db::actors::{Actor, NewRemoteActor};
use crate::federation;
use crate::state::AppState;
use activitypub::activity::{Accept, IncomingActivity};
use activitypub::signature as httpsig;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use uuid::Uuid;

pub async fn handler(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match process(&state, &method, &uri, &headers, &body).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "inbox request rejected");
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        }
    }
}

async fn process(
    state: &AppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> anyhow::Result<Response> {
    let signature_header = header_str(headers, "signature")
        .ok_or_else(|| anyhow::anyhow!("missing Signature header"))?;
    let digest_header =
        header_str(headers, "digest").ok_or_else(|| anyhow::anyhow!("missing Digest header"))?;
    httpsig::verify_digest(digest_header, body)?;

    let raw: Value = serde_json::from_slice(body)?;
    let activity: IncomingActivity = serde_json::from_value(raw.clone())?;

    let sender_ap_id =
        httpsig::extract_key_id(signature_header).map(|key_id| strip_key_fragment(&key_id))?;
    anyhow::ensure!(
        sender_ap_id == activity.actor,
        "Signature keyId does not match activity actor"
    );

    let sender_domain = domain_of(&sender_ap_id)?;
    if crate::db::domain_blocks::is_blocked(&state.pool, &sender_domain).await? {
        return Ok((StatusCode::FORBIDDEN, "domain blocked").into_response());
    }

    let sender = resolve_actor(state, &sender_ap_id).await?;

    let request_headers = header_pairs(headers);
    httpsig::verify(
        &sender.public_key_pem,
        signature_header,
        method.as_str(),
        uri.path(),
        &request_headers,
    )?;

    let is_new =
        crate::db::activities_log::record_inbound(&state.pool, &activity.id, &raw, Some(sender.id))
            .await?;
    if !is_new {
        return Ok(StatusCode::ACCEPTED.into_response());
    }

    match activity.kind.as_str() {
        "Follow" => handle_follow(state, &sender, &activity).await?,
        "Undo" => handle_undo(state, &sender, &activity).await?,
        other => tracing::info!(kind = other, "ignoring unsupported activity type"),
    }

    Ok(StatusCode::ACCEPTED.into_response())
}

async fn handle_follow(
    state: &AppState,
    sender: &Actor,
    activity: &IncomingActivity,
) -> anyhow::Result<()> {
    let followee_ap_id = activity
        .object
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Follow object must be an actor URL"))?;
    let followee = crate::db::actors::find_by_ap_id(&state.pool, followee_ap_id)
        .await?
        .filter(|actor| actor.is_local)
        .ok_or_else(|| anyhow::anyhow!("Follow target is not a local actor: {followee_ap_id}"))?;

    crate::db::follows::upsert_accepted(&state.pool, sender.id, followee.id, &activity.id).await?;

    let private_key_pem = followee
        .private_key_pem
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("local actor is missing a private key"))?;
    let key_id = format!("{}#main-key", followee.ap_id);
    let accept = Accept::new(
        format!("{}#accepts/follows/{}", followee.ap_id, Uuid::new_v4()),
        &followee.ap_id,
        activity,
    );
    let body = serde_json::to_value(&accept)?;

    if let Err(error) = federation::deliver(
        &state.http,
        private_key_pem,
        &key_id,
        &sender.inbox_url,
        &body,
    )
    .await
    {
        // Best-effort for phase 2 — no retry queue yet (that's phase 3's
        // delivery_queue, SPEC.md §3.4). The follow is still recorded.
        tracing::warn!(%error, followee = %followee.ap_id, follower = %sender.ap_id, "failed to deliver Accept");
    }

    Ok(())
}

async fn handle_undo(
    state: &AppState,
    sender: &Actor,
    activity: &IncomingActivity,
) -> anyhow::Result<()> {
    let inner_kind = activity.object.get("type").and_then(Value::as_str);
    if inner_kind != Some("Follow") {
        tracing::info!(?inner_kind, "ignoring Undo of unsupported activity type");
        return Ok(());
    }
    let followee_ap_id = activity
        .object
        .get("object")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Undo(Follow) is missing the followed actor URL"))?;
    let followee = crate::db::actors::find_by_ap_id(&state.pool, followee_ap_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("unknown follow target: {followee_ap_id}"))?;

    crate::db::follows::remove(&state.pool, sender.id, followee.id).await?;
    Ok(())
}

/// Looks up a cached copy of the actor, or fetches and caches it if
/// this is the first time we've seen them (SPEC.md §3.3).
async fn resolve_actor(state: &AppState, ap_id: &str) -> anyhow::Result<Actor> {
    if let Some(actor) = crate::db::actors::find_by_ap_id(&state.pool, ap_id).await? {
        return Ok(actor);
    }

    let person = federation::fetch_remote_actor(&state.http, ap_id).await?;
    anyhow::ensure!(person.id == ap_id, "fetched actor id does not match");
    let domain = domain_of(&person.id)?;

    let actor = crate::db::actors::upsert_remote(
        &state.pool,
        NewRemoteActor {
            ap_id: person.id,
            preferred_username: person.preferred_username,
            domain,
            display_name: person.name,
            inbox_url: person.inbox,
            outbox_url: person.outbox,
            followers_url: person.followers,
            following_url: person.following,
            public_key_pem: person.public_key.public_key_pem,
        },
    )
    .await?;
    Ok(actor)
}

fn strip_key_fragment(key_id: &str) -> String {
    key_id.split('#').next().unwrap_or(key_id).to_string()
}

fn domain_of(ap_id: &str) -> anyhow::Result<String> {
    let uri: Uri = ap_id.parse()?;
    Ok(uri
        .host()
        .ok_or_else(|| anyhow::anyhow!("actor id has no host: {ap_id}"))?
        .to_string())
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

fn header_pairs(headers: &HeaderMap) -> Vec<(&str, &str)> {
    headers
        .iter()
        .filter_map(|(name, value)| Some((name.as_str(), value.to_str().ok()?)))
        .collect()
}
