# --- frontend: build static assets -----------------------------------
FROM node:22-slim AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# --- backend: plan + cache dependencies (cargo-chef) ------------------
# NOTE: pin an exact cargo-chef/rust tag before production use — this
# uses the floating "latest-rust-1" tag documented at
# https://github.com/LukeMathWalker/cargo-chef for local/dev builds.
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

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
# TODO(later phase): serve ./frontend/dist as the SPA fallback from the
# Axum router — see SPEC.md §11 open decision on SSR vs. plain HTML.

ENV RUST_LOG=info
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/brillion"]
CMD ["serve"]
