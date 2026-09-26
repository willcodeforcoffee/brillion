# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Brillion is a multi-user blogging platform that is also a full ActivityPub
client and server (federates with Mastodon/Pleroma/etc.), with a
self-hosted OAuth 2.0 provider and RustFS (S3-compatible) media storage.

**`SPEC.md` is the source of truth for design decisions** — domain model,
every route/endpoint, the GraphQL schema shape, the OAuth flow, the
Postgres schema, and open decisions with recommendations. Section numbers
in code comments (e.g. `SPEC.md §3.4`) refer to it; when in doubt about
*why* something is structured a certain way, read the referenced section
before changing it. Current section map:

1. Overview · 2. Domain model · 3. ActivityPub/ActivityStreams ·
4. Blogging features (public pages, RSS/Atom) · 5. Media storage
(RustFS) · 6. GraphQL API · 7. OAuth 2.0 provider · 8. CLI ·
9. Data model (Postgres) · 10. Deployment (Docker Compose) ·
11. Frontend (React) · 12. Open decisions · 13. Suggested phasing

The project is being built in the phases listed in §13. **Currently in
phase 1** (local-only blog): users/actors are real and Postgres-backed
via the CLI; posts, the GraphQL API, OAuth, image upload, and the public
pages/feeds are not yet implemented. Federation (phases 2-4) hasn't
started — the `activitypub` crate's `activity`/`object`/`signature`
modules are mostly still TODO stubs beyond keypair generation.

## Layout

```
backend/       Rust (Axum) server + CLI binary, package name `brillion`
activitypub/   AS2/ActivityPub types + HTTP Signatures (no HTTP framework dep)
frontend/      React + TypeScript (Vite) client
deploy/        Reverse proxy config (Caddyfile)
docker-compose.yml, Dockerfile
```

`backend` depends on `activitypub` as a path dependency. `activitypub` has
no Axum/HTTP dependency by design — AS2 types, HTTP Signatures, and actor
URL conventions live there so they aren't coupled to the web framework,
in prep for federation logic that must be testable in isolation.

Inside `backend/src`: `main.rs` (CLI dispatch + `serve`), `cli.rs` (clap
argument definitions), `config.rs` (env-based `Config`), `crypto.rs`
(Argon2id password hashing), `router.rs` (Axum router — currently just
`/health`), `db/` (Postgres access: `mod.rs` has pool connect + migration
runner, `users.rs`/`actors.rs` are per-table query modules).

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
