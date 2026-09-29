//! End-to-end test of outbound federation (SPEC.md §3.4): a mock remote
//! actor follows our real (in-process) server's local actor, who then
//! publishes, edits, and deletes a post — verifying a real signed
//! `Create`/`Update`/`Delete` arrives at the follower's inbox each time,
//! and that the outbox collection reflects a published post.
//!
//! Requires a live, disposable Postgres database. Not run by default,
//! and **do not run via `cargo test` with a `DATABASE_URL=...` shell
//! override** — if this repo uses mise (it does), `cargo` is a shim
//! that silently discards that override and re-reads `.env`'s real
//! `DATABASE_URL` instead, so the test would write into the real
//! database (this happened for real once — see CLAUDE.md). Build once,
//! then run the compiled binary directly, which isn't shimmed:
//!
//!   cargo build --workspace --tests
//!   DATABASE_URL=postgres://user:pass@host/disposable_db \
//!     ./target/debug/deps/outbox_federation-<hash> --ignored

use activitypub::urls::ActorUrls;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use brillion::config::Config;
use brillion::db;
use brillion::db::posts::{NewPost, PostEdit};
use brillion::delivery;
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
/// server's inbox at `path` — used here just to get bob's `Follow`
/// accepted (and his actor cached) the same real way any remote
/// follower would, before we test the outbound side.
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
async fn publish_edit_delete_deliver_to_a_follower() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("set DATABASE_URL to a disposable test database before running this test");

    let pool = db::connect(&database_url).await.unwrap();
    db::run_migrations(&pool).await.unwrap();

    // --- our server ("alice"), bound to an ephemeral port --------------
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let our_addr = listener.local_addr().unwrap();
    let public_base_url = format!("http://{our_addr}");

    let config = Config {
        database_url: database_url.clone(),
        public_base_url: public_base_url.clone(),
        port: our_addr.port(),
        ..Config::test_default()
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

    let state = AppState::new(pool.clone(), config).unwrap();
    let app = brillion::router::app(state.clone());
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });

    // --- mock remote follower ("bob"), also in-process ------------------
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
        .route("/users/{username}", get(mock_actor))
        .route("/users/{username}/inbox", post(mock_inbox))
        .with_state(mock_state);
    tokio::spawn(async move {
        axum::serve(mock_listener, mock_app).await.unwrap();
    });

    // --- bob follows alice, the same real way any remote follower would
    let client = reqwest::Client::new();
    let follow_body = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{bob_ap_id}#follows/{}", Uuid::new_v4()),
        "type": "Follow",
        "actor": bob_ap_id,
        "object": alice.ap_id,
    });
    let response = send_signed_activity(
        &client,
        our_addr,
        &format!("/users/{username}/inbox"),
        &bob_ap_id,
        &bob_keypair.private_key_pem,
        &follow_body,
    )
    .await;
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
    received.lock().unwrap().clear(); // drop the Accept we just got sent

    // --- publish a post -------------------------------------------------
    let post = db::posts::create(
        &pool,
        NewPost {
            actor_id: alice.id,
            title: "Hello, Fediverse",
            summary: Some("a test post"),
            body_markdown: "hello",
            body_html: "<p>hello</p>",
        },
    )
    .await
    .unwrap();
    let permalink = format!("{public_base_url}/users/{username}/{}", post.slug);
    let post = db::posts::publish(&pool, post.id, &permalink)
        .await
        .unwrap();
    delivery::enqueue_create(&pool, &alice, &post)
        .await
        .unwrap();
    delivery::process_due(&state).await.unwrap();

    let after_create = received.lock().unwrap().clone();
    assert_eq!(after_create.len(), 1, "expected one delivered Create");
    assert_eq!(after_create[0]["type"], "Create");
    assert_eq!(after_create[0]["actor"], alice.ap_id);
    assert_eq!(after_create[0]["object"]["type"], "Article");
    assert_eq!(after_create[0]["object"]["id"], permalink);
    assert_eq!(after_create[0]["object"]["name"], "Hello, Fediverse");
    assert!(
        after_create[0]["object"].get("@context").is_none(),
        "embedded object shouldn't carry its own @context"
    );
    received.lock().unwrap().clear();

    // --- outbox reflects the published post ------------------------------
    let outbox: Value = client
        .get(format!("http://{our_addr}/users/{username}/outbox"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(outbox["totalItems"], 1);
    assert_eq!(outbox["orderedItems"][0], format!("{permalink}#create"));

    // --- edit the post: Update -------------------------------------------
    let post = db::posts::update(
        &pool,
        post.id,
        PostEdit {
            title: "Hello, Fediverse (edited)",
            summary: Some("a test post"),
            body_markdown: "hello, edited",
            body_html: "<p>hello, edited</p>",
        },
    )
    .await
    .unwrap();
    delivery::enqueue_update(&pool, &alice, &post)
        .await
        .unwrap();
    delivery::process_due(&state).await.unwrap();

    let after_update = received.lock().unwrap().clone();
    assert_eq!(after_update.len(), 1, "expected one delivered Update");
    assert_eq!(after_update[0]["type"], "Update");
    assert_eq!(
        after_update[0]["object"]["name"],
        "Hello, Fediverse (edited)"
    );
    received.lock().unwrap().clear();

    // --- delete the post: Delete(Tombstone) -------------------------------
    delivery::enqueue_delete(&pool, &alice, &permalink)
        .await
        .unwrap();
    db::posts::delete(&pool, post.id).await.unwrap();
    delivery::process_due(&state).await.unwrap();

    let after_delete = received.lock().unwrap().clone();
    assert_eq!(after_delete.len(), 1, "expected one delivered Delete");
    assert_eq!(after_delete[0]["type"], "Delete");
    assert_eq!(after_delete[0]["object"]["type"], "Tombstone");
    assert_eq!(after_delete[0]["object"]["id"], permalink);

    // --- every delivery ended up marked 'delivered' ------------------------
    let (delivered_count,): (i64,) =
        sqlx::query_as("select count(*) from delivery_queue where status = 'delivered'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(delivered_count, 3, "Create + Update + Delete all delivered");
}
