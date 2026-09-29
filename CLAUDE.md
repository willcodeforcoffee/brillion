# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Brillion is a multi-user blogging platform that is also a full ActivityPub
client and server (federates with Mastodon/Pleroma/etc.), with a
self-hosted OAuth 2.0 provider and RustFS (S3-compatible) media storage.

**`SPEC.md` is the source of truth for design decisions** — domain model,
every route/endpoint, the GraphQL schema shape, the OAuth flow, the
Postgres schema, and §12's decision log (was "open decisions"; all 10
items are now resolved — read it for the *why*, since some didn't go
with the original recommendation, e.g. the GraphQL client ended up
Apollo, not `urql`). Section numbers in code comments
(e.g. `SPEC.md §3.4`) refer to it; when in doubt about *why* something
is structured a certain way, read the referenced section before
changing it. Current section map:

1. Overview · 2. Domain model · 3. ActivityPub/ActivityStreams ·
4. Blogging features (public pages, RSS/Atom) · 5. Media storage
(RustFS) · 6. GraphQL API · 7. OAuth 2.0 provider · 8. CLI ·
9. Data model (Postgres) · 10. Deployment (Docker Compose) ·
11. Frontend (React) · 12. Decisions (formerly open) · 13. Suggested phasing

The project is being built in the phases listed in §13, but not strictly
in order — phase 2 (federation inbound) was implemented before the rest
of phase 1, since it turned out to depend only on actors, not posts.
**Current state: phases 1-3 are done.** Users/actors, posts
(CRUD + publish, Markdown → sanitized HTML), the GraphQL API, a
self-hosted OAuth 2.0 provider (PKCE), RustFS image upload, the
server-rendered public pages, and RSS/Atom feeds are all implemented
and verified end-to-end against real Postgres (and RustFS, for images)
— not just unit-tested. So is federation-inbound (WebFinger, actor
documents, inbox receiving `Follow`/`Accept`/`Undo` with signature
verification and rate limiting, followers/following) and
federation-outbound (`Create`/`Update`/`Delete` delivered to followers
on publish/edit/delete, with retry/backoff via an in-process
`delivery_queue` worker — `backend/src/delivery.rs`,
`backend/tests/outbox_federation.rs`; SPEC.md §3.4). The React SPA is
now wired to the real backend too (Apollo Client, OAuth PKCE login,
post editor, admin, settings) — verified in a real Chrome browser, not
just typechecked. Local dev now runs the backend on the host (`cargo run
-- serve`, not the `backend` service in `docker-compose.yml`, which is
commented out) alongside `postgres`/`rustfs`/`proxy` in Compose — Caddy
routes `/` to `host.docker.internal:3000` and RustFS's `/media/*` gets
CORS headers and prefix-stripping it lacked before (see
`deploy/Caddyfile` and `backend/src/media.rs`); the Caddy layer is
needed for presigned image uploads to work at all from a browser, even
though the backend itself isn't containerized. **Known real gaps, not
assumptions:** RustFS's bucket isn't actually public-read in practice
(`ensure_bucket`'s doc comment). Presigned upload itself is verified
correct via direct HTTP testing (curl/Python driving the real
OAuth/GraphQL flow: signature, CORS headers, and prefix-stripping all
check out consistently) but showed intermittent `503`s specifically
through this session's browser-automation tool on the identical URLs
that succeed every time outside it — see `MediaStore::presign_upload`'s
doc comment before assuming either "it's fixed" or "it's still broken"
without testing in an ordinary browser yourself. Outbound federation's
own known gap: delivery only reaches followers, not `@mentioned`
actors (needs an outbound WebFinger client that doesn't exist yet — see
`backend/src/delivery.rs`). Phase 4 (replies/likes/announces both
directions, timeline merging) hasn't started.

## Layout

```
backend/           Rust (Axum) server + CLI, package `brillion` (lib.rs + thin main.rs)
backend/templates/ askama templates for the server-rendered public pages
backend/tests/     Integration tests — DB-backed ones are #[ignore]d by default
activitypub/       AS2/ActivityPub types + HTTP Signatures (no HTTP framework dep)
frontend/          React + TypeScript (Vite) client — authenticated app only
deploy/            Reverse proxy config (Caddyfile)
docker-compose.yml, Dockerfile
```

`backend` depends on `activitypub` as a path dependency. `activitypub` has
no Axum/HTTP dependency by design — AS2 types, HTTP Signatures, and actor
URL conventions live there so they aren't coupled to the web framework,
in prep for federation logic that must be testable in isolation.

`backend` is a library (`lib.rs`) plus a thin `main.rs` — done
specifically so `backend/tests/*.rs` integration tests can build the
real `router::app(state)` in-process and drive it with a real HTTP
client, rather than shelling out to a built binary.

Inside `backend/src`: `main.rs` (CLI dispatch + `serve`), `cli.rs` (clap
argument definitions), `config.rs` (env-based `Config`), `crypto.rs`
(Argon2id password hashing + fast token hashing for OAuth, distinct
functions — see Architecture notes), `markdown.rs` (Markdown → HTML via
`pulldown-cmark`, then `ammonia`-sanitized), `media.rs` (`MediaStore`:
RustFS presigned upload URLs, bucket creation), `pages.rs` (shared view
model for the server-rendered pages), `state.rs` (`AppState` = pool +
config + reqwest client + `MediaStore` + the built `async-graphql`
schema), `federation.rs` (outbound HTTP: fetch a remote actor's Person
doc, sign+deliver an activity to a remote inbox), `router.rs` (mounts
everything onto `AppState`; also where the inbox rate-limit layer
lives), `graphql/` (`Query`/`Mutation`/`types` — the client-facing API,
§6), `routes/` (one file per route/route-group: `webfinger`, `nodeinfo`,
`actor`, `inbox`, `collections`, `graphql`, `oauth`, `pages` (homepage),
`post` (permalink), `feeds`), `db/` (Postgres access, one module per
table: `users`, `actors`, `posts`, `media_attachments`, `follows`,
`domain_blocks`, `activities_log`, `oauth`).

Inside `frontend/src`: `App.tsx` wires up `react-router` (component API
— `BrowserRouter`/`Routes`/`Route`, not the data-router) over
`routes/Layout.tsx` (nav + `Outlet`) and one placeholder page per
`routes/*Page.tsx` (`Home`, `PostEditor`, `Settings`, `Login`,
`OAuthCallback`, `AdminUsers`, `NotFound`) — real data-fetching isn't
wired up yet (no GraphQL client installed), these are routing
scaffolding only. This SPA is the **authenticated app only** — public
pages (homepage, actor profile, post permalink) are server-rendered by
`backend` instead (§11/§12 #2), so there are deliberately no React
routes for `/users/:username` etc. GraphQL client is Apollo Client (§11/§12,
decided over the lighter-weight `urql`) — not yet added as a
dependency.

## Commands

### Toolchain

`mise.toml` pins Rust and Node and auto-loads `.env`. Run `mise install`
once; with `mise activate` in your shell rc, `cd`-ing into the repo loads
both — otherwise run `eval "$(mise env)"` manually.

### Backend (Rust workspace: `backend` + `activitypub`)

```sh
cargo build --workspace
cargo test --workspace                       # single test: cargo test -p brillion <test_name>
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                              # cargo fmt --all -- --check to verify only
```

Running the server/CLI needs Postgres and `DATABASE_URL`/`PUBLIC_BASE_URL`
in the environment (via mise + `.env`, or exported by hand). Local dev
runs the backend on the host, not the `backend` service in
`docker-compose.yml` (commented out — faster iteration than rebuilding
the Docker image on every change), alongside `postgres`/`rustfs`/`proxy`
in Compose:

```sh
docker compose up -d postgres rustfs proxy   # or point DATABASE_URL at your own instance
cargo run -p brillion -- migrate
cargo run -p brillion -- user create --email you@example.com --username you --role admin
cargo run -p brillion -- user list
cargo run -p brillion -- serve               # GET /health on :3000; also runs pending migrations on boot
```

`proxy` (Caddy) is needed even though `backend` isn't containerized:
`deploy/Caddyfile` routes `/` to `host.docker.internal:3000` (the host
process) and adds the CORS headers + path-prefix-stripping RustFS's
`/media/*` needs (§5.1) — without it, image upload fails from a browser
even though everything else works fine hitting `:3000` directly.

`sqlx` queries are written with the runtime-checked API
(`sqlx::query_as::<_, T>(...)`), not the `sqlx::query!` compile-time
macros — deliberately, so `cargo build` and the Docker build never need a
live database connection. Keep new queries in this style.

DB-backed integration tests (currently `backend/tests/inbox_federation.rs`)
are marked `#[ignore]` so plain `cargo test` never needs a database. Run
them explicitly against a **disposable** database:

```sh
DATABASE_URL=postgres://user:pass@host/db \
  cargo test -p brillion --test inbox_federation -- --ignored
```

Note `sqlx-cli`-style compile-time query macros aren't in play here, so
there's no `.sqlx` offline cache to regenerate.

### Frontend

```sh
cd frontend
npm install
cp .env.example .env   # VITE_API_BASE — see note below
npm run dev
npm run build     # tsc -b && vite build — this is the typecheck step
npm run lint       # oxlint
```

`VITE_API_BASE` picks which backend the SPA talks to: `http://localhost:3000`
to hit the host-run backend directly (fine for everything except image
upload — RustFS isn't reachable at all without going through Caddy), or
`http://localhost` to go through Caddy on `:80`, which is needed for
image upload and is the default recommendation.

### Full stack

```sh
cp .env.example .env   # then fill in real secrets
docker compose up -d postgres rustfs proxy   # backend runs on the host — see above
cargo run -p brillion -- serve
```

The frontend still runs separately (`npm run dev`, not part of Compose)
with `VITE_API_BASE=http://localhost`.

To containerize the backend too (e.g. to test the actual `Dockerfile`,
or match production more closely): uncomment the `backend` service in
`docker-compose.yml`, flip `deploy/Caddyfile`'s catch-all route back to
`reverse_proxy backend:3000` (from `host.docker.internal:3000`), stop
the host `cargo run -- serve` process, and `docker compose up --build`.

### CI

`.github/workflows/ci.yml` runs, on every push/PR: `cargo fmt --check` /
`clippy --all-targets -- -D warnings` / `test` for the backend, and
`lint` / `build` for the frontend. Match these locally before pushing.

## Architecture notes

- **Three separate route surfaces are planned, on purpose** (§3 vs §4 vs
  §6): the ActivityPub federation surface (inbox/outbox, actor documents,
  content-negotiated JSON-LD) is AS2 *activities*; the public blog pages
  and RSS/Atom feeds are plain *objects* with no ActivityPub wrapper; the
  GraphQL API (`/graphql`) is the client-facing surface for the React
  app. All three read from the same `posts`/`actors` tables — don't let
  federation concepts leak into the feed/homepage rendering path or vice
  versa.
- **Public pages are server-rendered HTML, not part of the React SPA**
  (§11/§12 #2) — implemented via `askama` (`backend/templates/`),
  `routes::pages::homepage`, `routes::actor::handler`'s non-AS2-`Accept`
  branch, and `routes::post::handler`. The React router deliberately has
  no routes for these paths — don't add `/users/:username` etc. back to
  `frontend/src/App.tsx`.
- **Local actors always get an RSA keypair and inbox/outbox/follower
  URLs at creation time** (`activitypub::signature::generate_keypair`,
  `activitypub::urls::ActorUrls`), even though federation isn't wired up
  yet — this is intentional per §8, so the CLI's `user create` code path
  doesn't need to change shape once phase 2 lands.
- **User + actor creation is transactional** (`backend/src/main.rs`
  `user_command`, `UserCommand::Create` arm) — a `User` row always has a
  corresponding local `Actor` row; don't add a code path that creates one
  without the other.
- **`Config::domain()`** (`backend/src/config.rs`) derives the
  `user@domain` federation address from `PUBLIC_BASE_URL` rather than a
  separately configured value, so they can't drift out of sync.
- **RustFS specifics were verified against the real image**, not
  guessed: S3 API on `:9000`, console on `:9001`, and it honors
  `RUSTFS_ROOT_USER`/`RUSTFS_ROOT_PASSWORD` (also accepts
  `RUSTFS_ACCESS_KEY`/`SECRET_KEY` or Minio-style names). See §5.
- **`DATABASE_URL` in `.env` is for host-side tools only**
  (`cargo run` outside Compose) — it's a literal value kept in sync by
  hand with the `POSTGRES_*` vars, not a variable expansion; Postgres is
  additionally exposed on the host via `POSTGRES_PORT` (default 5432) for
  this reason. The `backend` service inside Compose builds its own
  `DATABASE_URL` from `POSTGRES_*` using the internal `postgres` hostname
  and ignores the `.env` literal.
- **Migrations run automatically on `serve` startup**, not just via the
  explicit `migrate` subcommand (both call the same
  `db::run_migrations`) — this was a deliberate choice from §10 (option
  1: auto-migrate on boot) over a separate one-shot migration service.
- **On axum 0.8: path params use `{name}`, not `:name`** — the project
  moved from axum 0.7 to 0.8 (to match `async-graphql-axum`'s
  requirement) mid-phase. The old `:name` syntax silently registers as a
  *literal* path segment instead of erroring, so a wrong-syntax route
  just 404s on every real request. Bit us twice already (once on 0.7,
  once forgetting to flip back on the 0.8 upgrade); check this first if
  a route "isn't found."
- **HTTP Signatures use `draft-cavage` (RSA-SHA256 + PKCS#1v1.5)**, not
  RFC 9421 — `activitypub::signature` — because that's what's actually
  deployed across the Fediverse. Signing/verifying goes through
  `rsa::Pkcs1v15Sign` directly (not the `pkcs1v15::SigningKey`/`Signer`
  trait wrappers) because those require `sha2::Sha256: AssociatedOid`,
  which needs `sha2`'s `oid` feature explicitly enabled — see the
  `features = ["oid"]` on the `sha2` dep in `activitypub/Cargo.toml`.
- **Actors have an explicit `ap_id` column** (their AS2 `id` URL) rather
  than reconstructing it from `domain`/`preferred_username` — required
  for remote actors, whose `id` isn't guaranteed to follow our own
  `{base}/users/{name}` convention. `resolve_actor` in
  `routes/inbox.rs` looks a sender up by `ap_id` first, and only fetches
  + caches (`federation::fetch_remote_actor` +
  `db::actors::upsert_remote`) on a cache miss — using the activity's
  own `actor` field directly, not WebFinger (WebFinger is only for
  resolving a human-typed `user@domain`, not for key lookup).
- **Inbound `Follow` gets a synchronous, best-effort `Accept` delivery**
  (`routes/inbox.rs` `handle_follow`) — no retry queue yet (that's
  phase 3's `delivery_queue`, §3.4/§9). A delivery failure is logged but
  doesn't fail the inbox response or roll back the recorded follow.
- **Dedup is by the activity's own AS2 `id`** (`activities_log`,
  unique on `ap_id`) — `record_inbound` returns `false` for an
  already-seen id and the handler short-circuits to `202` without
  reprocessing. Exercised directly in the integration test (replaying
  the original `Follow` after its `Undo` must NOT recreate the follow).
- **Sandbox-specific gotcha (not a Brillion bug):** in this dev
  environment, `docker exec <container> psql ...` has been observed to
  return **stale or entirely different data** than a real TCP connection
  to the same container's published port — reproduced multiple times
  across different containers. Don't trust `docker exec` for verifying
  database state; connect over TCP instead (`cargo run -- user list`,
  or a throwaway Rust binary under `backend/examples/` using
  `sqlx::PgPoolOptions`, deleted after use). Also: distinct throwaway
  containers on distinct host ports have, at least once, resolved to
  the *same* backing Postgres over TCP — when isolation actually
  matters, prefer unique row-level identifiers (random suffixes) over
  a separate container/port/dbname, and clean up explicitly afterward.
- **`mise` shims silently override shell exports for any var also in
  `.env`.** Confirmed: `export PUBLIC_BASE_URL=http://localhost:9999`
  in a shell, then `cargo run` in the same shell — the process sees
  `PUBLIC_BASE_URL=http://localhost` (the `.env` value) instead, even
  though a plain `bash -c 'echo $PUBLIC_BASE_URL'` in that same shell
  correctly shows the override. Every mise-managed tool invocation
  (`cargo`, `npm`, ...) goes through a shim that re-resolves `mise.toml`
  + `.env` and apparently takes `.env`'s value over whatever the calling
  shell already exported, for any key `.env` also defines (currently:
  `DATABASE_URL`, `PUBLIC_BASE_URL`, `S3_BUCKET`, `POSTGRES_*`,
  `RUSTFS_*`, `RUST_LOG`, `HTTP_PORT`/`HTTPS_PORT`). Env vars *not* in
  `.env` (e.g. `PORT`, `S3_ACCESS_KEY`) pass through shell exports
  unaffected — that's how per-test port overrides still worked while
  `PUBLIC_BASE_URL` overrides silently didn't. Only applies to
  **mise-shimmed tools** (`cargo`, `npm`, ...) — running the compiled
  binary directly (`./target/debug/brillion serve`, not `cargo run`)
  isn't a shim at all and correctly inherits whatever the shell
  exported, `.env` keys included. To override one of `.env`'s own keys
  for a one-off `cargo`/`npm` invocation, edit `.env` itself (or
  temporarily rename it) rather than exporting in the shell.
  **This bit a real `DATABASE_URL` override for the `#[ignore]`d
  integration tests** (`inbox_federation.rs`, `outbox_federation.rs`) —
  `DATABASE_URL=postgres://.../disposable_db cargo test ... -- --ignored`
  silently ran against `.env`'s real `DATABASE_URL` instead, writing
  test rows into the real database. **Run the compiled test binary
  directly** to actually get an isolated run:
  `DATABASE_URL=postgres://.../disposable_db ./target/debug/deps/<test_name>-<hash> --ignored`
  (find the exact binary name via `ls target/debug/deps/ | grep <test_name>`,
  after a `cargo build --workspace --tests` to make sure it's current) —
  and verify afterward with a query against the *real* database, not
  just the disposable one, that nothing unexpected landed there.
- **`Dockerfile`'s builder and runtime stages must share a Debian
  release, or the binary won't start.** Hit this for real: the builder
  used to be `lukemathwalker/cargo-chef:latest-rust-1` (a floating tag),
  which drifted to a newer glibc than `debian:bookworm-slim` (the
  runtime stage) ships. `docker build` succeeded either way — the
  failure only showed up when actually *running* the image
  (`GLIBC_2.38' not found`). Fixed by building `cargo-chef` from
  `rust:1-bookworm` instead of the floating tag, matching runtime's
  Debian release. **Lesson: `docker build` succeeding is not sufficient
  verification for this Dockerfile — always also run the built image**
  (`docker run ... brillion-backend:tag`, hit `/health`) before calling
  a Docker-related change done. If the Rust version ever needs bumping,
  keep both stages on the same Debian release.
- **Password hashing vs. token hashing are deliberately different
  algorithms** (`backend/src/crypto.rs`): `hash_password`/
  `verify_password` use Argon2id (slow, randomly salted — for
  low-entropy user-chosen passwords). `generate_token`/`hash_token` use
  plain SHA-256 (fast, deterministic — for high-entropy random OAuth
  client secrets/codes/tokens, where the goal is O(1) lookup-by-hash,
  not brute-force resistance a slow hash would add nothing to). Don't
  reach for `hash_password` when adding new token types.
- **GraphQL auth is a `Viewer` in the async-graphql context, not a
  session** (`graphql::Viewer`, resolved per-request in
  `routes::graphql::resolve_viewer` from the `Authorization: Bearer`
  header). `Viewer::require_actor()` is the standard guard at the top of
  any mutation that needs a signed-in author — see any mutation in
  `graphql/mutation.rs` for the pattern.
- **Image upload is a two-step handshake, not a single call**
  (`graphql/mutation.rs` `request_image_upload` / `attach_image` /
  `update_actor_image`): `requestImageUpload` inserts an *unattached*
  `media_attachments` row (`post_id = NULL`) and returns its id alongside
  the presigned URL; the browser `PUT`s the bytes directly to RustFS;
  only then does `attachImage`/`updateActorImage` link that row to a
  post or actor. Byte size/dimensions are never actually verified
  server-side in this flow (no HEAD-the-object step) — a known
  simplification, not an oversight.
- **RustFS's bucket is not actually public-read**, despite §12 #7's
  decision and `MediaStore` requesting the `x-amz-acl: public-read`
  canned ACL on both bucket and object creation. Verified with a real
  create-bucket → upload → anonymous-GET round trip against
  `rustfs/rustfs:latest`: still 403. Likely running with S3 "object
  ownership enforced" (ACLs disabled in favor of bucket policies), which
  `rusty-s3` has no built-in action for. See the long comment on
  `MediaStore::ensure_bucket` before attempting to "fix" this — it's
  already been tried and documented, not just deferred.
- **The inbox rate limiter needs `into_make_service_with_connect_info`**
  (`main.rs` `serve`, and the integration test's server setup) — without
  it, `tower_governor`'s `SmartIpKeyExtractor` has no peer address to
  fall back to when there's no `X-Forwarded-For` header (e.g. hitting
  the backend directly rather than through Caddy) and every request
  500s. Confirmed by the integration test failing this exact way until
  `connect_info` was added.
- **Direct-to-browser presigned image upload needs Caddy in front of
  RustFS** (`docker-compose.yml`'s `proxy` service, `deploy/Caddyfile`)
  — `cargo run -- serve` alone, without the proxy, cannot serve this
  feature: RustFS isn't published to the host at all by design (only
  reachable via the proxy or the internal Docker network), and even if
  it were, RustFS sends no `Access-Control-Allow-*` headers on its own.
  This is independent of whether `backend` itself runs on the host or
  in Docker — Caddy's `/media/*` route only talks to RustFS either way;
  see its catch-all route for how it reaches `backend`, which does
  depend on that. `MediaStore` (§5.1) signs
  presigned PUT URLs against the *public* origin (`Config::public_base_url`)
  rather than RustFS's internal one, then manually prepends `/media` onto
  the already-signed URL's path — the signature itself only covers the
  bucket-rooted path RustFS will actually see once Caddy's `/media/*`
  route strips that prefix back off (`handle_path`, not `handle` —
  `handle` alone would forward the unstripped path and mismatch
  RustFS's bucket-rooted paths). That same Caddy route adds the CORS
  headers RustFS doesn't. **Verified correct via direct HTTP testing**
  (curl and a Python script driving the real OAuth/GraphQL flow —
  signature validates, CORS headers present, repeatable) but **not
  cleanly verified from an ordinary browser**: testing through this
  session's browser-automation tool showed intermittent `503`s on URLs
  that succeed every time via curl run immediately after, on both
  RustFS's internal engine and a genuinely fresh `File` object dispatched
  via a real `change` event (i.e., not an artifact of that tool's
  synthetic file-picker simulation specifically) — the failure
  correlates with "request came from that automated tab," not with
  anything about the request itself, which is why it reads as a
  tooling/environment quirk rather than an app bug, but don't take that
  as settled without testing in a real browser. See the doc comment on
  `MediaStore::presign_upload` for the full reasoning.
- **The frontend's OAuth login is a full-page redirect, not a fetch**
  (`frontend/src/lib/auth.ts` `startLogin`/`completeLogin`) — matching
  how `/oauth/authorize` actually works: it's a server-rendered
  `askama` login form (email/password), not a JSON endpoint, so the
  SPA can't drive it via XHR. The SPA self-registers as an OAuth app on
  first use (`POST /api/v1/apps`, cached in `localStorage` — fine for a
  single-tenant instance) and stores the PKCE `code_verifier`/`state`
  in `sessionStorage` across the redirect. `backend/src/router.rs`'s
  `CorsLayer` (default-allowed origin `http://localhost:5173`, override
  via `FRONTEND_ORIGIN`) only needs to cover `/graphql`, `/oauth/token`,
  and `/api/v1/apps` — never `/oauth/authorize` itself, since that's a
  real browser navigation, not a cross-origin fetch.
- **Outbound activity ids are derived, never stored** (`backend/src/delivery.rs`):
  a post's `ap_object_id` (its permalink, set once at first publish —
  `db::posts::publish`'s `coalesce`) is the only federation identity
  actually persisted. `Create`'s id is always `{permalink}#create`
  (stable — the outbox route derives the same string to list a post's
  activity, `db::posts::list_published_ap_ids` + `#create` in
  `routes::collections::outbox`, rather than reading a separate
  activities table). `Update`/`Delete` ids embed a timestamp
  (`{permalink}#update-{unix ts}` / `#delete-{unix ts}`) since those
  need to be unique per edit/delete event, unlike `Create` which only
  ever happens once per post.
- **Embedded objects inside `Create`/`Update` drop their own `@context`**
  (`delivery::article_json` strips it from the serialized `Article`
  before embedding) — nested `@context` is legal JSON-LD but pointless
  noise once the wrapping activity already declares one. `Tombstone`
  (used in `Delete`) never had one to begin with — see its doc comment.
- **`delivery_queue` has no `'in_progress'` status** — `db::delivery_queue::claim_due`
  instead gives a claimed row a 5-minute lease by pushing
  `next_attempt_at` forward, then `mark_delivered`/`mark_failed`
  corrects it. If the worker crashes mid-delivery, the lease just
  expires and the next tick retries — no separate crash-recovery path
  needed. Uses `FOR UPDATE SKIP LOCKED` so this stays correct if
  `run_worker` is ever scaled beyond the current single in-process task
  (§10/§12: no separate `worker` service, at least until delivery
  volume warrants it).
- **A real `DATABASE_URL=... cargo test ...` override silently no-ops**
  in this repo (mise shim, see the `mise` gotcha above) — bit outbound
  federation testing for real: an `#[ignore]`d integration test run
  believed to target a disposable database actually wrote rows into the
  real one. Always run these via the compiled binary directly (see the
  doc comment atop `inbox_federation.rs`/`outbox_federation.rs`), and
  verify against the *real* database afterward that nothing unexpected
  landed there — don't just trust that the override worked.
