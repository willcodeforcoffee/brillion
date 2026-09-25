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

Requires a Rust toolchain (stable) — not installed in the environment
this scaffold was generated in, so `cargo build`/`test`/`clippy` have
**not** been run against this code yet. Run these before trusting it:

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
```

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

The `rustfs` image/port in `docker-compose.yml` are placeholders pending
a check against [RustFS's own docs](https://rustfs.com) — verify before
relying on them (see SPEC.md §5 and the `NOTE:` comments in
`docker-compose.yml` / `.env.example`).

## CI

`.github/workflows/ci.yml` runs `cargo fmt --check` / `clippy` / `test`
for the backend and `lint` / `build` (typecheck) for the frontend on
every push and PR.
