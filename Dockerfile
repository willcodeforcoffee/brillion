# --- frontend: build static assets -----------------------------------
FROM node:22-slim AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# --- backend: plan + cache dependencies (cargo-chef) ------------------
# Built on the same Debian release as the `runtime` stage below
# (`bookworm`), deliberately *not* `lukemathwalker/cargo-chef`'s
# floating `latest-rust-1` tag — that resolved to a newer glibc than
# `debian:bookworm-slim` ships, so the compiled binary failed to start
# in `runtime` at all (`GLIBC_2.38' not found`). Confirmed by actually
# running the built image, not just building it. If the Rust version
# ever needs bumping, keep both stages on a matching Debian release.
FROM rust:1-bookworm AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY Cargo.toml ./
COPY backend backend
COPY activitypub activitypub
# Works whether or not Cargo.lock is committed (see .gitignore).
RUN cargo generate-lockfile
RUN cargo chef prepare --recipe-path recipe.json

# --- backend: build ----------------------------------------------------
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml ./
COPY --from=planner /app/Cargo.lock ./
COPY backend backend
COPY activitypub activitypub
RUN cargo build --release --bin brillion

# --- runtime -------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/brillion /usr/local/bin/brillion
COPY --from=frontend-builder /app/frontend/dist ./frontend/dist
# Public pages (homepage, actor profile, post permalink) are server-
# rendered by `backend` itself (askama templates, SPEC.md §11/§12 #2) —
# this ./frontend/dist copy is for the authenticated SPA only (login,
# settings, editor, admin). TODO: no static-file-serving fallback wired
# up in the Axum router yet to actually serve it.

ENV RUST_LOG=info
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/brillion"]
CMD ["serve"]
