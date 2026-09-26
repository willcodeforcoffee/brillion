# Brillion

A multi-user blogging platform that is a full ActivityPub client and
server. See [`SPEC.md`](./SPEC.md) for the full design — domain model,
federation protocol, GraphQL API, OAuth provider, media storage, and the
phased build plan.

Status: **phase 1** (local-only blog) has users/actors (Postgres-backed,
via the CLI); posts, the GraphQL API, OAuth, image upload, and the
public pages/feeds are still to come. **Phase 2** (federation inbound)
is done: WebFinger, actor documents, inbox receiving
`Follow`/`Accept`/`Undo` with HTTP Signature verification, and
followers/following collections all work against real Postgres and a
mock remote server (see `backend/tests/inbox_federation.rs`). Phases
3-4 (outbound federation, replies/likes/announces) haven't started.

## Layout

```
backend/       Rust (Axum) server + CLI, package `brillion` (lib.rs + thin main.rs)
backend/tests/ Integration tests (DB-backed ones are #[ignore]d by default)
activitypub/   AS2/ActivityPub types + HTTP Signatures (no HTTP framework dep)
frontend/      React + TypeScript (Vite) client
deploy/        Reverse proxy config (Caddyfile)
docker-compose.yml
Dockerfile     Multi-stage build: frontend assets + backend binary
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
docker compose up -d postgres   # or point DATABASE_URL at your own instance

cargo run -p brillion -- migrate
cargo run -p brillion -- user create --email you@example.com --username you --role admin
cargo run -p brillion -- user list
cargo run -p brillion -- serve             # GET /health on :3000; also runs pending migrations on boot
```

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

## Frontend

```sh
cd frontend
npm install
npm run dev
```

## Full stack (Docker Compose)

```sh
cp .env.example .env   # then fill in real secrets
docker compose up --build
```

Verified working end-to-end: all four services come up (postgres
healthy, rustfs, backend, caddy proxy); the backend connects to
Postgres and runs migrations automatically on boot; the proxy routes
`/health` to `backend` and `/media/*` to `rustfs` (`:9000`; console on
`:9001`, honoring `RUSTFS_ROOT_USER`/`RUSTFS_ROOT_PASSWORD`); and
`docker compose exec backend brillion user create ...` works against
the containerized Postgres exactly as it does locally.

## CI

`.github/workflows/ci.yml` runs `cargo fmt --check` / `clippy` / `test`
for the backend and `lint` / `build` (typecheck) for the frontend on
every push and PR.
