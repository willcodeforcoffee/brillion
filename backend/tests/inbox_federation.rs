//! End-to-end test of inbound federation (SPEC.md §3.3): a mock remote
//! actor sends a signed `Follow` to our real (in-process) server, we
//! verify the signature, cache the remote actor, record the follow,
//! and deliver a signed `Accept` back — then the same for `Undo`.
//!
//! Requires a live, disposable Postgres database. Not run by default:
//!
//!   DATABASE_URL=postgres://user:pass@host/db \
//!     cargo test -p brillion --test inbox_federation -- --ignored

use activitypub::urls::ActorUrls;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use brillion::config::Config;
use brillion::db;
use brillion::state::AppState;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
struct MockRemote {
    received: Arc<Mutex<Vec<Value>>>,
    ap_id: String,
    username: String,
    public_key_pem: String,
}

async fn mock_actor(State(remote): State<MockRemote>) -> Json<Value> {
    Json(json!({
        "@context": ["https://www.w3.org/ns/activitystreams", "https://w3id.org/security/v1"],
        "id": remote.ap_id,
        "type": "Person",
        "preferredUsername": remote.username,
        "name": "Bob",
        "summary": "",
        "inbox": format!("{}/inbox", remote.ap_id),
        "outbox": format!("{}/outbox", remote.ap_id),
        "followers": format!("{}/followers", remote.ap_id),
        "following": format!("{}/following", remote.ap_id),
        "publicKey": {
            "id": format!("{}#main-key", remote.ap_id),
            "owner": remote.ap_id,
            "publicKeyPem": remote.public_key_pem,
        },
    }))
}

async fn mock_inbox(State(remote): State<MockRemote>, Json(body): Json<Value>) -> StatusCode {
    remote.received.lock().unwrap().push(body);
    StatusCode::ACCEPTED
}

/// Signs `body` as `signer` (a remote actor) and POSTs it to our
/// server's inbox at `path`, returning the response.
async fn send_signed_activity(
    client: &reqwest::Client,
    our_addr: std::net::SocketAddr,
    path: &str,
    signer_ap_id: &str,
    signer_private_key_pem: &str,
    body: &Value,
) -> reqwest::Response {
    let body_bytes = serde_json::to_vec(body).unwrap();
    let digest = activitypub::signature::compute_digest(&body_bytes);
    let date = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string();
    let host = our_addr.to_string();
    let headers = [
        ("host", host.as_str()),
        ("date", date.as_str()),
        ("digest", digest.as_str()),
    ];
    let key_id = format!("{signer_ap_id}#main-key");
    let signature =
        activitypub::signature::sign(signer_private_key_pem, &key_id, "post", path, &headers)
            .unwrap();

    client
        .post(format!("http://{our_addr}{path}"))
        .header("Date", date)
        .header("Digest", digest)
        .header("Signature", signature)
        .header("Content-Type", "application/activity+json")
        .body(body_bytes)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires a live, disposable Postgres — see module docs"]
async fn follow_then_undo_round_trip() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("set DATABASE_URL to a disposable test database before running this test");

    let pool = db::connect(&database_url).await.unwrap();
    db::run_migrations(&pool).await.unwrap();

    // --- our server, bound to an ephemeral port ---------------------
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let our_addr = listener.local_addr().unwrap();
    let public_base_url = format!("http://{our_addr}");

    let config = Config {
        database_url: database_url.clone(),
        public_base_url: public_base_url.clone(),
        port: our_addr.port(),
    };

    let username = format!("alice-{}", Uuid::new_v4().simple());
    let keypair = activitypub::signature::generate_keypair().unwrap();
    let urls = ActorUrls::new(&public_base_url, &username);
    let user = db::users::create(
        &pool,
        &format!("{username}@example.com"),
        "unused-hash",
        db::users::Role::Author,
    )
    .await
    .unwrap();
    let alice = db::actors::create_local(
        &pool,
        db::actors::NewLocalActor {
            user_id: user.id,
            preferred_username: &username,
            domain: &config.domain(),
            urls,
            public_key_pem: keypair.public_key_pem,
            private_key_pem: keypair.private_key_pem,
        },
    )
    .await
    .unwrap();

    let state = AppState::new(pool.clone(), config);
    let app = brillion::router::app(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // --- mock remote actor ("bob"), also in-process -------------------
    // A unique username per run: `domain_of()` only extracts the host
    // (127.0.0.1), not the ephemeral port, so re-running this test
    // against the same persistent database would otherwise collide on
    // actors' (preferred_username, domain) uniqueness.
    let mock_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = mock_listener.local_addr().unwrap();
    let bob_username = format!("bob-{}", Uuid::new_v4().simple());
    let bob_ap_id = format!("http://{mock_addr}/users/{bob_username}");
    let bob_keypair = activitypub::signature::generate_keypair().unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));

    let mock_state = MockRemote {
        received: received.clone(),
        ap_id: bob_ap_id.clone(),
        username: bob_username.clone(),
        public_key_pem: bob_keypair.public_key_pem.clone(),
    };
    let mock_app = Router::new()
        .route("/users/:username", get(mock_actor))
        .route("/users/:username/inbox", post(mock_inbox))
        .with_state(mock_state);
    tokio::spawn(async move {
        axum::serve(mock_listener, mock_app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let inbox_path = format!("/users/{username}/inbox");

    // --- Follow -------------------------------------------------------
    let follow_id = format!("{bob_ap_id}#follows/{}", Uuid::new_v4());
    let follow_body = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": follow_id,
        "type": "Follow",
        "actor": bob_ap_id,
        "object": alice.ap_id,
    });

    let response = send_signed_activity(
        &client,
        our_addr,
        &inbox_path,
        &bob_ap_id,
        &bob_keypair.private_key_pem,
        &follow_body,
    )
    .await;
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);

    let (follow_state,): (String,) = sqlx::query_as(
        "select f.state from follows f
         join actors a on a.id = f.follower_actor_id
         where a.ap_id = $1",
    )
    .bind(&bob_ap_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(follow_state, "accepted");

    let received_after_follow = received.lock().unwrap().clone();
    assert_eq!(
        received_after_follow.len(),
        1,
        "expected one delivered Accept"
    );
    assert_eq!(received_after_follow[0]["type"], "Accept");
    assert_eq!(received_after_follow[0]["object"]["type"], "Follow");
    assert_eq!(received_after_follow[0]["object"]["actor"], bob_ap_id);

    // --- Undo(Follow) --------------------------------------------------
    let undo_body = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{bob_ap_id}#undo/{}", Uuid::new_v4()),
        "type": "Undo",
        "actor": bob_ap_id,
        "object": {
            "id": follow_id,
            "type": "Follow",
            "actor": bob_ap_id,
            "object": alice.ap_id,
        },
    });
    let response = send_signed_activity(
        &client,
        our_addr,
        &inbox_path,
        &bob_ap_id,
        &bob_keypair.private_key_pem,
        &undo_body,
    )
    .await;
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);

    let (remaining,): (i64,) = sqlx::query_as(
        "select count(*) from follows f
         join actors a on a.id = f.follower_actor_id
         where a.ap_id = $1",
    )
    .bind(&bob_ap_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0, "follow should be removed after Undo");

    // --- replaying the original Follow must be a no-op (dedup) --------
    let response = send_signed_activity(
        &client,
        our_addr,
        &inbox_path,
        &bob_ap_id,
        &bob_keypair.private_key_pem,
        &follow_body,
    )
    .await;
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);

    let (remaining,): (i64,) = sqlx::query_as(
        "select count(*) from follows f
         join actors a on a.id = f.follower_actor_id
         where a.ap_id = $1",
    )
    .bind(&bob_ap_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        remaining, 0,
        "replayed activity id must be deduped, not reprocessed"
    );
}
