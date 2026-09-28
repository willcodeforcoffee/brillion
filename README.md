# Brillion

A multi-user blogging platform that is a full ActivityPub client and
server. See [`SPEC.md`](./SPEC.md) for the full design — domain model,
federation protocol, GraphQL API, OAuth provider, media storage, and the
phased build plan.

Status: **phase 1** (local-only blog) and **phase 2** (federation
inbound) are both done. Posts (CRUD + publish, Markdown → sanitized
HTML), the GraphQL API (Apollo-ready schema — `me`/`post`/`posts`/
`actor`, `createPost`/`updatePost`/`publishPost`/`deletePost`,
`requestImageUpload`/`attachImage`/`updateActorImage`), a self-hosted
OAuth 2.0 provider (authorization code + PKCE, `/api/v1/apps`,
`/oauth/*`), RustFS image upload (presigned PUT, browser → RustFS
directly), server-rendered public pages (homepage, actor profile, post
permalink — a client-only SPA was tried and reverted, see SPEC.md
§12 #2), and RSS/Atom feeds all work end-to-end against real Postgres
and RustFS. WebFinger, actor documents, inbox
(`Follow`/`Accept`/`Undo` with HTTP Signature verification and rate
limiting), and followers/following collections work against real
Postgres and a mock remote server (`backend/tests/inbox_federation.rs`).
The React SPA is now wired to the real backend (Apollo Client, OAuth
PKCE login, post editor, admin user list, settings) — see "Frontend"
below. Local dev runs `backend` on the host with `postgres`/`rustfs`/
`proxy` (Caddy) in Docker Compose — Caddy is still needed even so,
since presigned image upload requires it (RustFS isn't published to the
host outside the proxy) — see "Full stack" below.
RustFS's bucket still isn't actually public-read in practice (a real,
verified gap — see `backend/src/media.rs`). Phases 3-4 (outbound
federation, replies/likes/announces) haven't started.

## Layout

```
backend/           Rust (Axum) server + CLI, package `brillion` (lib.rs + thin main.rs)
backend/templates/ askama templates for the server-rendered public pages
backend/tests/     Integration tests (DB-backed ones are #[ignore]d by default)
activitypub/       AS2/ActivityPub types + HTTP Signatures (no HTTP framework dep)
frontend/          React + TypeScript (Vite) client — authenticated app only
deploy/            Reverse proxy config (Caddyfile)
docker-compose.yml
Dockerfile         Multi-stage build: frontend assets + backend binary
```

## Toolchain

`mise.toml` pins Rust and Node (matching the versions this project is
built/tested against) and auto-loads `.env`. Run `mise install` once;
with `mise activate` in your shell rc, `cd`-ing into this directory
loads both automatically — otherwise run `eval "$(mise env)"` yourself.

## Backend

Requires a Rust toolchain (stable), a Postgres database, and
`DATABASE_URL`/`PUBLIC_BASE_URL` in the environment — via mise loading
`.env` (see Toolchain above), or exported by hand if you're not using
mise. Verified working: `cargo build`, `test`,
`clippy --all-targets -- -D warnings`, and `fmt --check` all pass clean.

```sh
docker compose up -d postgres rustfs proxy # or point DATABASE_URL at your own instance

cargo run -p brillion -- migrate
cargo run -p brillion -- user create --email you@example.com --username you --role admin
cargo run -p brillion -- user list
cargo run -p brillion -- serve             # GET /health on :3000; also runs pending migrations on boot
```

`proxy` (Caddy) is worth running even though `backend` itself runs on
the host, not in Docker: `deploy/Caddyfile` routes `/` to
`host.docker.internal:3000` and adds the CORS headers RustFS's
`/media/*` needs for image upload to work from a browser at all — see
"Full stack" below.

`user create` hashes the password with Argon2id, generates the local
actor's RSA keypair, and inserts the user + actor in one transaction —
verified end-to-end against a real Postgres, including that a failure
(e.g. duplicate email) rolls back cleanly with no orphaned actor row.

Federation (WebFinger, actor documents, inbox, followers/following) is
covered by `backend/tests/inbox_federation.rs`, an in-process
integration test: it spins up the real router plus a mock remote actor
on ephemeral ports, sends a genuinely signed `Follow`, and asserts the
signature verifies, the actor gets cached, the follow is recorded, a
signed `Accept` is delivered back, `Undo` removes it, and a replayed
activity id is deduped. Skipped by default (no DB in CI); run with:

```sh
DATABASE_URL=postgres://user:pass@host/db \
  cargo test -p brillion --test inbox_federation -- --ignored
```

## GraphQL, OAuth, images, pages, feeds

All verified end-to-end against a real Postgres (and, for images, a
real RustFS) with a throwaway test user cleaned up afterward — not just
compiled. The full loop exercised: register an OAuth app
(`POST /api/v1/apps`) → PKCE authorize/token flow → `me`/`createPost`/
`publishPost` over `POST /graphql` with the resulting bearer token →
anonymous `posts`/`post` queries and a rejected anonymous mutation →
`requestImageUpload` → an actual presigned `PUT` to RustFS → the
homepage/actor-profile/post-permalink pages rendering that data →
`/feed.rss` and `/feed.atom` (site-wide and per-author) as valid XML.

- **GraphQL** (`/graphql`, `GET` serves GraphiQL in dev): `backend/src/graphql/`.
  Auth is `Authorization: Bearer <token>` → `Viewer` in the resolver
  context (`routes/graphql.rs`); anonymous requests get read-only access.
- **OAuth** (`/api/v1/apps`, `/oauth/{authorize,token,revoke}`):
  `backend/src/routes/oauth.rs` + `db/oauth.rs`. Authorization code +
  PKCE (S256) only — no `password` grant.
- **Images**: `requestImageUpload` returns a presigned RustFS PUT URL
  the browser hits directly (through the Caddy proxy's `/media/*`,
  required — see "Full stack" below); `attachImage`/`updateActorImage`
  link the uploaded object afterward. **Known gap** (verified, not
  assumed): RustFS doesn't honor the `x-amz-acl: public-read` header on
  bucket or object creation — uploaded images are not actually publicly
  readable yet. See the note in `backend/src/media.rs`.
- **Pages** (`/`, `/users/:username`, `/users/:username/:slug`):
  server-rendered via `askama` (`backend/templates/`) — not part of the
  React SPA (SPEC.md §12 #2).
- **Feeds** (`/feed.{rss,atom}`, `/users/:username/feed.{rss,atom}`):
  `backend/src/routes/feeds.rs`, full post content per §12 #6.
- **Rate limiting**: the inbox routes carry a `tower_governor` layer
  (`SmartIpKeyExtractor`, so it reads real client IPs through the Caddy
  proxy rather than rate-limiting the proxy's own IP for everyone).

## Frontend

```sh
cd frontend
npm install
cp .env.example .env   # sets VITE_API_BASE — see note below
npm run dev
```

`VITE_API_BASE` picks which backend the SPA talks to. Use
`http://localhost:3000` to hit a `cargo run -- serve` backend directly
(fine for everything except image upload); use `http://localhost` to go
through Caddy on `:80` instead (needed for image upload — see "Full
stack" below), and is the default recommendation either way.

Verified end-to-end in a real Chrome browser against the real backend +
Postgres (with a throwaway admin user created via the CLI and cleaned
up afterward): OAuth self-registration (`POST /api/v1/apps`) and PKCE
login redirect through the backend's own `/oauth/authorize` form,
`createPost`/`publishPost` from the post editor, the timeline and admin
user list queries, and the server-rendered permalink page for a post
published through the SPA. Apollo Client (`@apollo/client` v4) talks to
`/graphql` with the stored bearer token attached via `SetContextLink`.

Avatar/header upload in Settings: `requestImageUpload` → presigned PUT
→ `attachImage`/`updateActorImage` is verified correct via direct HTTP
testing (curl and a Python script driving the real OAuth/GraphQL flow —
signature validates, CORS headers present, repeatable), but showed
intermittent `503`s specifically when driven through this session's
browser-automation tool on URLs that succeed every time via curl run
immediately after — see the doc comment on `MediaStore::presign_upload`
in `backend/src/media.rs` before assuming it's either fixed or still
broken; it hasn't been confirmed clean in an ordinary browser.

## Full stack

The recommended local setup: `postgres`/`rustfs`/`proxy` (Caddy) in
Docker Compose, `backend` on the host (`docker-compose.yml`'s `backend`
service is commented out — faster iteration than rebuilding the Docker
image every change):

```sh
cp .env.example .env   # then fill in real secrets
docker compose up -d postgres rustfs proxy
cargo run -p brillion -- serve
```

Caddy is needed even with `backend` on the host: `deploy/Caddyfile`
routes `/` to `host.docker.internal:3000` (Docker Desktop resolves this
to the host automatically) and RustFS's `/media/*` gets the CORS
headers and path-prefix-stripping (`handle_path`, since RustFS's own
paths are bucket-rooted with no `/media` segment) that image upload
needs — without Caddy in front, RustFS isn't reachable from a browser
at all. Verified: `/health`, `/`, `/media/*` (CORS headers present,
correct path after stripping) all work through the proxy this way.

**Fully containerized alternative** (e.g. to test the actual
`Dockerfile`, or match production more closely): uncomment the
`backend` service in `docker-compose.yml`, flip `deploy/Caddyfile`'s
catch-all route back to `reverse_proxy backend:3000` (from
`host.docker.internal:3000`), stop the host `cargo run -- serve`
process, then `docker compose up --build`. Verified working end-to-end,
including actually *running* the built image (not just `docker build`
succeeding) — migrations run, and `/health`/`/`/`/nodeinfo/2.1` all
respond correctly. This caught a real bug: the `Dockerfile`'s builder
stage used to be `lukemathwalker/cargo-chef`'s floating `latest-rust-1`
tag, which had drifted to a newer glibc than the `debian:bookworm-slim`
runtime stage ships — the binary built fine but failed to even start
(`GLIBC_2.38' not found`). Fixed by building `cargo-chef` from
`rust:1-bookworm` instead, matching the runtime stage's Debian release.
Also verified: `docker compose exec backend brillion user create ...`
works against the containerized Postgres exactly as it does locally.

## CI

`.github/workflows/ci.yml` runs `cargo fmt --check` / `clippy` / `test`
for the backend and `lint` / `build` (typecheck) for the frontend on
every push and PR.
