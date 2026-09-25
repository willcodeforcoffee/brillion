# Brillion

A multi-user blogging platform that is a full ActivityPub client and
server. See [`SPEC.md`](./SPEC.md) for the full design — domain model,
federation protocol, GraphQL API, OAuth provider, media storage, and the
phased build plan. This repo is currently at **phase 0** (scaffold).

## Layout

```
backend/       Rust (Axum) server + CLI binary, package name `brillion`
activitypub/   AS2/ActivityPub types + HTTP Signatures (no HTTP framework dep)
frontend/      React + TypeScript (Vite) client
deploy/        Reverse proxy config (Caddyfile)
docker-compose.yml
Dockerfile     Multi-stage build: frontend assets + backend binary
```

## Backend

Requires a Rust toolchain (stable). Verified working: `cargo build`,
`test`, `clippy --all-targets -- -D warnings`, and `fmt --check` all pass
clean on this scaffold.

```sh
cargo run -p brillion -- serve            # GET /health on :3000
cargo run -p brillion -- user create --email you@example.com --username you
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

Verified working end-to-end: `docker compose up --build` brings up all
four services (postgres healthy, rustfs, backend, caddy proxy); the
proxy correctly routes `/health` to `backend` and `/media/*` to `rustfs`
on its S3 API port (`:9000`; console on `:9001`), honoring
`RUSTFS_ROOT_USER`/`RUSTFS_ROOT_PASSWORD`. Note the backend doesn't
actually talk to Postgres or RustFS at runtime yet — that lands in
phase 1 — so this only confirms the container topology and networking,
not application-level integration.

## CI

`.github/workflows/ci.yml` runs `cargo fmt --check` / `clippy` / `test`
for the backend and `lint` / `build` (typecheck) for the frontend on
every push and PR.
