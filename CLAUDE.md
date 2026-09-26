# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Brillion is a multi-user blogging platform that is also a full ActivityPub
client and server (federates with Mastodon/Pleroma/etc.), with a
self-hosted OAuth 2.0 provider and RustFS (S3-compatible) media storage.

**`SPEC.md` is the source of truth for design decisions** — domain model,
every route/endpoint, the GraphQL schema shape, the OAuth flow, the
Postgres schema, and §12's decision log (was "open decisions"; all 10
items are now resolved — read it for the *why*, since several didn't
go with the original recommendation, e.g. public pages ended up a
client-only SPA, not server-rendered). Section numbers in code comments
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
**Current state:** phase 1's users/actors (Postgres-backed, via the
CLI) and phase 2's federation-inbound (WebFinger, actor documents,
inbox receiving `Follow`/`Accept`/`Undo` with signature verification,
followers/following) are both done. Still missing from phase 1: posts,
the GraphQL API, OAuth, image upload, public pages/feeds. Phases 3-4
(outbound federation beyond the auto-`Accept`, replies/likes/announces)
haven't started.

## Layout

```
backend/       Rust (Axum) server + CLI, package `brillion` (lib.rs + thin main.rs)
backend/tests/ Integration tests — DB-backed ones are #[ignore]d by default
activitypub/   AS2/ActivityPub types + HTTP Signatures (no HTTP framework dep)
frontend/      React + TypeScript (Vite) client
deploy/        Reverse proxy config (Caddyfile)
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
(Argon2id password hashing), `state.rs` (`AppState` = pool + config +
reqwest client), `federation.rs` (outbound HTTP: fetch a remote actor's
Person doc, sign+deliver an activity to a remote inbox), `router.rs`
(mounts everything onto `AppState`), `routes/` (one file per
route/route-group: `webfinger`, `nodeinfo`, `actor`, `inbox`,
`collections`), `db/` (Postgres access, one module per table: `users`,
`actors`, `follows`, `domain_blocks`, `activities_log`).

Inside `frontend/src`: `App.tsx` wires up `react-router` (component API
— `BrowserRouter`/`Routes`/`Route`, not the data-router) over
`routes/Layout.tsx` (nav + `Outlet`) and one placeholder page per
`routes/*Page.tsx` (`Home`, `ActorProfile`, `Post`, `PostEditor`,
`Settings`, `Login`, `OAuthCallback`, `AdminUsers`, `NotFound`) — real
data-fetching isn't wired up yet (no GraphQL client installed), these
are routing scaffolding only. GraphQL client is Apollo Client (§11/§12,
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
in the environment (via mise + `.env`, or exported by hand):

```sh
docker compose up -d postgres                # or point DATABASE_URL at your own instance
cargo run -p brillion -- migrate
cargo run -p brillion -- user create --email you@example.com --username you --role admin
cargo run -p brillion -- user list
cargo run -p brillion -- serve               # GET /health on :3000; also runs pending migrations on boot
```

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
npm run dev
npm run build     # tsc -b && vite build — this is the typecheck step
npm run lint       # oxlint
```

### Full stack

```sh
cp .env.example .env   # then fill in real secrets
docker compose up --build
```

Note: `docker-compose.yml` currently has the `backend` and `proxy`
services commented out (local dev is running Postgres/RustFS in Docker
and the backend/frontend natively via `cargo run`/`npm run dev`) — check
its current state before assuming the full stack is wired up.

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
- **Public pages are a client-only SPA, decided over server-rendered
  HTML** (§11/§12) — `backend/src/routes/actor.rs`'s non-AS2-`Accept`
  branch is currently a placeholder HTML string; per the decision it
  should eventually serve the built frontend's `index.html` instead
  (letting `react-router` take over client-side), not grow real Axum
  HTML templating. Known accepted gap: no OpenGraph/crawler content
  until that JS bundle loads.
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
- **Axum 0.7 path params use `:name`, not `{name}`** — the latter is
  0.8+ syntax and silently registers as a *literal* path segment instead
  of erroring, so a wrong-syntax route just 404s on every real request.
  Bit us once already; check this first if a route "isn't found."
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
