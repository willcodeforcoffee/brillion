//! Outbound HTTP for federation: fetching a remote actor's Person
//! document (to cache their public key/inbox) and delivering signed
//! activities to a remote inbox. See SPEC.md §3.3/§3.4.

use activitypub::object::Person;
use activitypub::signature as httpsig;
use chrono::Utc;
use http::Uri;

const USER_AGENT: &str = concat!("brillion/", env!("CARGO_PKG_VERSION"));

/// Fetches and parses a remote actor's Person document by their AS2
/// `id` URL (e.g. the `actor` field of an inbound activity).
pub async fn fetch_remote_actor(http: &reqwest::Client, ap_id: &str) -> anyhow::Result<Person> {
    let response = http
        .get(ap_id)
        .header("Accept", "application/activity+json")
        .header("User-Agent", USER_AGENT)
        .send()
        .await?
        .error_for_status()?;
    let person: Person = response.json().await?;
    Ok(person)
}

/// Signs and delivers `body` to `inbox_url`, authenticating as the
/// local actor identified by `key_id` (its own `{ap_id}#main-key`).
pub async fn deliver(
    http_client: &reqwest::Client,
    private_key_pem: &str,
    key_id: &str,
    inbox_url: &str,
    body: &serde_json::Value,
) -> anyhow::Result<()> {
    let uri: Uri = inbox_url.parse()?;
    let host = uri
        .authority()
        .ok_or_else(|| anyhow::anyhow!("inbox URL has no host: {inbox_url}"))?
        .as_str()
        .to_string();
    let path = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string();

    let body_bytes = serde_json::to_vec(body)?;
    let digest = httpsig::compute_digest(&body_bytes);
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();

    let headers = [
        ("host", host.as_str()),
        ("date", date.as_str()),
        ("digest", digest.as_str()),
    ];
    let signature = httpsig::sign(private_key_pem, key_id, "post", &path, &headers)?;

    http_client
        .post(inbox_url)
        .header("Date", &date)
        .header("Digest", &digest)
        .header("Signature", signature)
        .header("Content-Type", "application/activity+json")
        .header("User-Agent", USER_AGENT)
        .body(body_bytes)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
