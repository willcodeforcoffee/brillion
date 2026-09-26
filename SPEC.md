# Brillion — Specification

## 1. Overview

**Brillion** is a multi-user blogging platform that is a full **ActivityPub** client and
server: posts, replies, likes, and follows federate with the wider
Fediverse (Mastodon, Pleroma, Akkoma, WriteFreely, etc.), while the blog
itself is served with a normal read/write web UI.

- **Backend:** Rust, [Axum](https://github.com/tokio-rs/axum) for HTTP,
  [async-graphql](https://async-graphql.github.io/async-graphql/en/index.html)
  for the first-party client API, [sqlx](https://github.com/launchbadge/sqlx)
  against Postgres.
- **Frontend:** React (Vite + TypeScript), talking to the backend over
  GraphQL, authenticating via the backend's own OAuth 2.0 provider.
- **Federation:** Full ActivityPub — WebFinger discovery, actor documents,
  signed inbox/outbox delivery, standard federation activities
  (Follow/Accept/Undo/Create/Update/Delete/Like/Announce).
- **Blogging:** Public, unauthenticated pages — a site-wide homepage and
  per-author pages listing published posts — plus RSS and Atom feeds
  (site-wide and per-author), built from the same post data as the
  ActivityPub outbox but exposed as ordinary blog content, not AS2
  activities.
- **Auth:** The server is its own OAuth 2.0 provider (authorization code +
  PKCE), in the shape of Mastodon's `/oauth/*` + `/api/v1/apps` — so
  third-party ActivityPub clients (e.g. mobile apps built for Mastodon's
  API shape) have a plausible integration path later, even though v1 only
  ships the first-party React app.
- **Media:** Post images and actor avatars/headers are uploaded directly
  from the browser to a [RustFS](https://rustfs.com) container (an
  S3-compatible object store) via presigned URLs, and served back out
  through the reverse proxy — the backend issues upload URLs but never
  proxies image bytes itself.
- **Deployment:** Docker Compose — Postgres, RustFS, backend (Axum,
  serves both GraphQL and ActivityPub endpoints, plus static frontend
  build or a separate frontend container), and a background delivery
  worker.
- **Admin:** A CLI bundled into the server binary (`brillion cli ...` or a
  separate `brillionctl` binary sharing the same crate) to create users,
  since there is no public registration flow in v1.

### Non-goals (v1)

- No media-heavy features (image galleries, video transcoding) beyond
  single attached images.
- No moderation tooling (reports, blocklists, instance-level defederation)
  beyond a basic actor/domain block list.
- No real-time streaming API (Mastodon's `/api/v1/streaming`) — the React
  app polls/refetches via GraphQL. Can be added later with GraphQL
  subscriptions over websockets (async-graphql supports this natively).
- No edit history / OCAP extensions beyond what ActivityPub's `Update`
  activity implies.

---

## 2. Domain model

### 2.1 Core entities

| Entity | Notes |
|---|---|
| `User` | Login identity: email, password hash (Argon2id), role. 1:1 with an `Actor`. |
| `Actor` | ActivityPub actor (`Person`). Has a keypair (RSA-2048, for HTTP Signatures), preferred username, display name, bio, avatar/header, inbox/outbox/followers/following URLs. Local actors have a `user_id`; remote actors (cached from federation) do not. |
| `Post` | A blog article. Maps to AS2 `Article` (long-form) — has title, slug, body (stored as sanitized HTML, authored as Markdown), summary, published/draft state, visibility. |
| `Reply` | A comment on a `Post` or another `Reply`. Maps to AS2 `Note`, threaded via `inReplyTo`. Can originate locally or federate in from remote actors replying to a local post. |
| `Follow` | Local actor follows another actor (local or remote), or a remote actor follows a local one. Has state: `pending` / `accepted`. |
| `Like` | Actor likes a `Post` or `Reply`. |
| `Announce` | Actor boosts/reshares a `Post`. |
| `MediaAttachment` | Single image attached to a `Post`. Stored in RustFS (S3-compatible), referenced by a public `/media/...` URL (§5). |
| `Activity` (inbox log) | Raw received/sent AS2 activities, kept for audit/debugging and idempotency (dedupe by activity `id`). |
| `OAuthApplication` | Registered client app (`client_id`/`client_secret`, redirect URIs, scopes) — mirrors Mastodon's `POST /api/v1/apps`. |
| `OAuthToken` | Issued access/refresh tokens, scoped, tied to a `User` and `OAuthApplication`. |
| `Domain block` | Blocked remote domains; inbox rejects activities from actors on blocked domains. |

### 2.2 Roles

- `admin` — full access, manages users via CLI/GraphQL.
- `author` — can create/edit/delete their own posts, has a public actor.
- `reader` (optional, v1.1) — authenticated account with no publishing
  rights, used only for following/liking/replying while logged into the
  local instance. v1 can ship with just `admin`/`author` and treat
  federated interactions from *remote* actors as needing no local account
  at all (which is how ActivityPub normally works).

---

## 3. ActivityPub / ActivityStreams

### 3.1 Discovery

- `GET /.well-known/webfinger?resource=acct:user@domain` → returns actor's
  ActivityPub `id` URL per [WebFinger](https://www.rfc-editor.org/rfc/rfc7033) / the ActivityPub discovery convention.
- `GET /.well-known/nodeinfo` → links to `GET /nodeinfo/2.1`, reporting
  software name/version, protocols (`activitypub`), and usage stats.
  Several Fediverse servers use NodeInfo presence as a sanity check before
  federating.
- `GET /.well-known/host-meta` — optional legacy XRD fallback, low
  priority.

### 3.2 Actors

- `GET /users/:username` — content-negotiated:
  - `Accept: application/activity+json` or `application/ld+json; profile="https://www.w3.org/ns/activitystreams"` → AS2 `Person` JSON-LD document (public key, inbox, outbox, followers, following URLs).
  - Otherwise → the SPA shell (`index.html`); the React app's client-side router (§11) renders the profile page from there.
- Each local actor has an RSA keypair generated at creation time (private
  key stored server-side only, public key embedded in the actor document)
  for HTTP Signatures.

### 3.3 Inbox / Outbox

- `POST /users/:username/inbox` — shared + per-actor inbox. Verifies HTTP
  Signature against the sending actor's public key (fetching/caching the
  remote actor document if unknown), checks domain block list, dedupes by
  activity `id`, then processes:
  - `Follow` → creates a pending `Follow` row, auto-accepts (v1: no
    follow-approval workflow) by sending an `Accept` back to the
    follower's inbox.
  - `Undo` (of `Follow`/`Like`/`Announce`) → reverses the corresponding
    row.
  - `Create` (`Note`, `inReplyTo` a local post/reply) → stores as a
    federated `Reply`.
  - `Like` / `Announce` (object = local post/reply) → stores as
    `Like`/`Announce`.
  - `Delete` → tombstones the referenced object if known.
  - Unknown/unsupported activity types are logged and ignored (HTTP 202).
- `POST /inbox` — shared inbox (recommended by the spec for delivery
  efficiency to multiple local recipients at once); resolves the
  intended local recipients from `to`/`cc`.
- `GET /users/:username/outbox` — paginated `OrderedCollection` of the
  actor's public activities (`Create` for each published post, `Like`,
  `Announce`), newest first.
- `GET /users/:username/followers`, `GET /users/:username/following` —
  paginated `OrderedCollection`s.

### 3.4 Outbound federation

- Publishing a post (`author` sets it to "published") creates a `Create`
  activity wrapping an `Article`, delivered to:
  - The author's followers' inboxes (shared inbox where advertised).
  - Any actors explicitly mentioned (`@user@domain` in the body).
- Edits → `Update` activity. Deletes → `Delete` (Tombstone).
- Local likes/replies/follows of remote actors/objects generate outbound
  `Like`/`Create`/`Follow` activities the same way.
- Delivery is **asynchronous**: activities are written to a
  `delivery_queue` table/job and processed by a background worker
  (separate Tokio task or separate container — see §10) with retry/backoff,
  since remote inboxes are unreliable. Requests are signed per
  [HTTP Signatures](https://datatracker.ietf.org/doc/html/draft-cavage-http-signatures) (`Signature` header, `keyId` = actor's public key URL,
  digest of body via `Digest: SHA-256=...`).

### 3.5 Object mapping

| Local concept | AS2 type |
|---|---|
| Published post | `Article` (with `content` as sanitized HTML, `name` as title, `summary`) |
| Comment/reply | `Note`, `inReplyTo` set |
| User | `Person` |
| Publish action | `Create` |
| Edit action | `Update` |
| Delete action | `Delete` (`Tombstone`) |
| Follow | `Follow` / `Accept` / `Reject` / `Undo` |
| Like | `Like` |
| Boost | `Announce` |
| Image attachment | `Image` (in `attachment` on a post; in `icon`/`image` on a `Person`) — see §5.4 |

### 3.6 Security

- HTTP Signatures required on all inbound federated requests (reject
  unsigned/invalid with 401).
- `Digest` header verified against body.
- JSON-LD is treated as plain JSON with a fixed, known `@context` (no
  general-purpose JSON-LD processor/expansion in v1 — matches what
  Mastodon itself does in practice) to avoid the complexity/attack surface
  of full JSON-LD processing.
- Actor key rotation: not in v1; a compromised key requires recreating the
  actor.
- Rate limiting on inbox endpoints (tower middleware) to blunt abuse from
  malicious/misbehaving remote servers.

---

## 4. Blogging features

### 4.1 Public pages

- `GET /` — site homepage: paginated, newest-first list of **published
  posts from all local users** (not just one author) — title, excerpt/
  summary, author byline + avatar, published date, link to the full post.
  **Decided (§11/§12): client-only SPA**, not server-rendered — this is
  the React router's `/` route (`HomePage`), fetching via GraphQL;
  independent of any single actor's outbox.
- `GET /users/:username` — per-author blog page: the same list, scoped to
  one actor's posts (this is the same URL as the content-negotiated AS2
  `Person` document — see §3.2 — the HTML branch of that content
  negotiation serves the SPA shell, which renders this page via
  `ActorProfilePage`).
- `GET /users/:username/:slug` — full post permalink. Content-negotiated:
  `Accept: application/activity+json` → the AS2 `Article` object (§3.5);
  otherwise → the SPA shell, rendering via `PostPage`.
- `GET /tags/:tag` (optional, v1.1) — posts filtered by tag; needs a
  `tags`/`post_tags` schema addition not otherwise required in v1 —
  deferred to v1.1 (§12).
- Pagination: `?page=N` (or a `?before=<post_id>` cursor) on all list
  pages, consistent with the GraphQL `posts` query's cursor pagination
  (§6.1).
- Every public HTML page includes `<link rel="alternate">` autodiscovery
  tags for its RSS/Atom equivalents (§4.2) and OpenGraph/Twitter Card meta
  tags for link previews.

### 4.2 RSS & Atom feeds

- `GET /feed.rss` (RSS 2.0) and `GET /feed.atom` (Atom 1.0) —
  **site-wide** feed of the most recent published posts across all local
  users, mirroring the homepage (§4.1).
- `GET /users/:username/feed.rss`, `GET /users/:username/feed.atom` —
  **per-author** feeds, scoped like the per-author HTML page.
- Feed items: title, permalink (`GET /users/:username/:slug`), publish
  date, author name, and the full sanitized post HTML — in
  `<content:encoded>` for RSS (Content module) and `<content type="html">`
  for Atom. `<guid isPermaLink="true">` / `<id>` is the permalink URL
  (not the AS2 object `id`), so feed readers get a plain HTTPS URL, not
  an ActivityPub identifier. Replies/comments are **not** included as
  feed items — those belong to ActivityPub, not the blog feed.
- Feed length: most recent 20 items, no pagination — the standard feed
  convention of readers polling periodically rather than paging through
  history.
- Implementation: the [`rss`](https://docs.rs/rss) crate for RSS 2.0 and
  [`atom_syndication`](https://docs.rs/atom_syndication) for Atom, both
  fed from the same query that backs the homepage/author page (§4.1) so
  the HTML, RSS, and Atom views can't drift out of sync.
- Feeds are regenerated/invalidated on post publish/update/delete rather
  than rebuilt per-request; a short `Cache-Control: max-age` is enough in
  v1 without a dedicated cache layer.

### 4.3 Relationship to ActivityPub

The blog's public pages/feeds and the ActivityPub outbox (§3.3) both
present a local actor's *published posts*, but are deliberately kept as
separate read models built from the same `posts` table:

- **Outbox** — an AS2 `OrderedCollection` of `Create`/`Update`/`Delete`/
  `Like`/`Announce` **activities**, consumed by federated software.
- **Feeds/homepage** — plain **objects**, no activity wrapper, consumed
  by RSS/Atom readers and human visitors — the format ordinary blog
  tooling expects, with no ActivityPub-specific concepts leaking in.

---

## 5. Media storage (RustFS)

Images (post attachments, actor avatars/headers) live in an
S3-compatible object store served by a [RustFS](https://rustfs.com)
container, not on the application container's local disk — keeps
`backend`/`worker` stateless and the storage backend swappable.

### 5.1 RustFS container

- `rustfs` service in Docker Compose: single Rust binary, S3-compatible
  API, own named volume for on-disk data (`rustfs_data:/data`).
- One bucket, e.g. `brillion-media` — created by `backend` on startup with an
  idempotent `create_bucket` call rather than a manual setup step.
- Credentials (access key/secret) supplied via env vars shared between
  `rustfs` and `backend`/`worker`, same `.env` pattern as the rest of the
  stack (§10).
- Talked to over the S3 API via the `aws-sdk-s3` crate, configured with a
  custom endpoint (RustFS's address) and path-style addressing. The same
  client code works against real S3/R2/MinIO in a non-Compose deployment
  by swapping the endpoint — RustFS is a drop-in for local/self-hosted use
  and isn't a hard dependency baked into application code.
- Two endpoint configs, one client: admin operations (bucket creation)
  use RustFS's Compose-internal address; **presigning** uses the public
  base URL (the blog's own domain + `/media`) so the signature matches
  what the browser will actually request through the proxy — otherwise
  presigned URLs signed against the internal hostname won't validate when
  hit from outside.

### 5.2 Upload flow

- `requestImageUpload` GraphQL mutation: the client asks the backend for
  a place to upload; the backend checks the caller owns the post/actor
  being edited, generates an object key (`posts/<post_id>/<uuid>.<ext>`
  or `actors/<actor_id>/avatar.<ext>`), and returns a **presigned PUT
  URL** (short-lived, scoped to that key) plus the final public URL.
- The browser uploads the file **directly to RustFS** via the presigned
  URL — image bytes never pass through the `backend` process. On
  completion, the client calls `attachImage`/`updateActor` with the
  returned key/URL to associate it with the post or actor.
- Constraints enforced server-side before issuing a presigned URL:
  allowed content types (`image/png`, `image/jpeg`, `image/webp`,
  `image/gif`), a max size (e.g. 8 MB), and one attachment per post in
  v1 (matches the "single attached image" non-goal already in §1).

### 5.3 Serving

- RustFS serves the objects directly. **Decided (§12): public-read
  bucket** (not presigned GET URLs) — the `backend`/`worker` containers
  are not in the read path for images.
- The reverse proxy (`caddy`/`nginx`, §10) fronts RustFS on a path like
  `/media/*` so images are served from the blog's own domain — needed so
  AS2 `Image` `url`s and OpenGraph image tags resolve to a first-party
  URL for federated software and link-preview crawlers, rather than a
  separate storage host.
- Stored URLs in Postgres (`media_attachments.url`, `actors.avatar_url`,
  `actors.header_url`) are always these public-facing `/media/...` URLs,
  never the raw RustFS/S3 endpoint.

### 5.4 ActivityPub representation

- Post images → AS2 `attachment` array on the `Article`/`Note`:
  `{"type": "Image", "mediaType": "image/jpeg", "url": "..."}`.
- Actor avatar/header → AS2 `icon` / `image` properties on the `Person`
  object, same shape.
- Federated (remote) images are **not** proxied/cached through RustFS —
  remote attachment URLs render as-is, pointing at the remote server.
  **Decided (§12): deferred to v1.1** (caching remote media locally, as
  Mastodon does, to avoid hotlinking and for reliability).

---

## 6. GraphQL API (async-graphql)

This is the **client-facing** API used by the React app — distinct from
the federation (AS2/REST) surface in §3, the public blog pages/feeds in
§4, and the media upload flow in §5. Served at `POST /graphql` (plus
`GET /graphql` for the GraphiQL/playground in non-production).

### 6.1 Representative schema shape

```graphql
type Query {
  me: User
  post(slug: String!, author: String!): Post
  posts(authorId: ID, first: Int, after: String): PostConnection!
  timeline(first: Int, after: String): PostConnection!   # local + followed remote actors
  actor(handle: String!): Actor                           # "user" or "user@domain.tld"
  search(query: String!): SearchResult!
}

type Mutation {
  createPost(input: CreatePostInput!): Post!
  updatePost(id: ID!, input: UpdatePostInput!): Post!
  publishPost(id: ID!): Post!
  deletePost(id: ID!): Boolean!

  createReply(postId: ID!, body: String!): Reply!

  requestImageUpload(input: ImageUploadRequest!): ImageUploadTarget!  # §5.2
  attachImage(postId: ID!, objectKey: String!, altText: String): Post!
  updateActorImage(kind: ActorImageKind!, objectKey: String!): Actor!

  follow(handle: String!): FollowResult!
  unfollow(handle: String!): Boolean!
  like(objectId: ID!): Boolean!
  unlike(objectId: ID!): Boolean!
  announce(objectId: ID!): Boolean!
}

type Subscription {
  notifications: Notification!   # v1.1 — likes/replies/follows on your posts
}
```

- Auth: `Authorization: Bearer <oauth access token>` resolved via a
  context extractor (Axum extension) that loads the `User` from the OAuth
  token table; anonymous requests get read-only access to public data.
- Mutations enforce ownership/role checks in resolvers, not just at the
  gateway.
- Pagination: Relay-style cursor connections (`async-graphql` has this
  built in via `connection::query`).

---

## 7. OAuth 2.0 provider

Mirrors the shape of Mastodon's client-auth API so the flow is familiar
and so third-party AP client apps have a known integration surface.

### 7.1 Endpoints

- `POST /api/v1/apps` — register a client application (name, redirect
  URI, scopes) → returns `client_id`/`client_secret`.
- `GET /oauth/authorize` — authorization endpoint; renders/redirects to
  the React app's login+consent screen; requires PKCE (`code_challenge`,
  `S256`) for public clients.
- `POST /oauth/token` — token endpoint; supports:
  - `authorization_code` grant (+ PKCE verification)
  - `refresh_token` grant
  - `password` grant is **not** offered (deprecated/insecure); the CLI
    uses a separate local mechanism (§8), not OAuth, to create users.
- `POST /oauth/revoke` — revoke a token.

### 7.2 Scopes

Mastodon-style coarse scopes to start: `read`, `write`, `follow`. Can be
split finer (`read:posts`, `write:posts`, etc.) later if a real
third-party client ecosystem shows up.

### 7.3 Tokens

- Opaque, random tokens (not JWT) stored hashed in Postgres, so revocation
  is immediate and doesn't require a blocklist. Access tokens are
  long-lived-ish (matches Mastodon's pattern of not expiring access
  tokens by default) with refresh tokens for rotation if we choose to add
  expiry later.
- Password hashing: Argon2id via the `argon2` crate.

---

## 8. CLI

Ships in the same binary as the server (`clap` subcommands), so the
Docker image needs no separate tooling:

```
brillion serve                      # run the HTTP server
brillion migrate                    # run pending sqlx migrations
brillion user create --email <e> --username <u> --role author   # prompts for password, or --password-stdin
brillion user list
brillion user set-role <username> <role>
brillion user reset-password <username>
brillion domain-block add <domain>
brillion domain-block remove <domain>
```

- `user create` generates the actor's RSA keypair, WebFinger-resolvable
  username, and (for `author`) an empty public actor document — same code
  path the (nonexistent, v1) signup flow would use, just invoked from the
  CLI instead of an HTTP handler.
- Run inside the container via `docker compose exec backend brillion user create ...`.

---

## 9. Data model (Postgres, sketch)

```
users(id, email, password_hash, role, created_at)
actors(id, user_id NULL, preferred_username, domain, display_name, bio,
       avatar_url, header_url, inbox_url, outbox_url, followers_url,
       following_url, public_key_pem, private_key_pem NULL, is_local, created_at)
posts(id, actor_id, slug, title, summary, body_html, body_markdown,
      status, ap_object_id, published_at, created_at, updated_at)
replies(id, actor_id, post_id, in_reply_to_id NULL, body_html, ap_object_id,
        created_at)
follows(id, follower_actor_id, followee_actor_id, state, created_at)
likes(id, actor_id, object_type, object_id, ap_activity_id, created_at)
announces(id, actor_id, post_id, ap_activity_id, created_at)
media_attachments(id, post_id, object_key, url, media_type, byte_size,
                   width, height, alt_text, created_at)
activities_log(id, direction, activity_json, actor_id NULL, processed_at, error NULL)
delivery_queue(id, activity_json, target_inbox_url, attempts, next_attempt_at, status)
oauth_applications(id, name, client_id, client_secret_hash, redirect_uris, scopes)
oauth_tokens(id, application_id, user_id, token_hash, refresh_token_hash,
             scopes, created_at, expires_at NULL, revoked_at NULL)
oauth_authorization_codes(id, application_id, user_id, code_hash,
                           code_challenge, redirect_uri, expires_at, used)
domain_blocks(id, domain, reason, created_at)
```

Migrations managed with `sqlx migrate`.

---

## 10. Deployment (Docker Compose)

Services:

- `postgres` — official `postgres:16` image, named volume for data.
- `rustfs` — RustFS image, named volume for object data (§5.1); exposes
  its S3 API only on the Compose-internal network (`backend`/`worker`
  talk to it there), with the `proxy` service (below) the only path to
  it from outside, at `/media/*`.
- `backend` — Rust binary (`brillion`), multi-stage Dockerfile
  (`cargo chef` for build caching). Runs `brillion migrate && brillion serve`
  on start (or migrations as a separate one-shot `entrypoint` service
  that `backend` `depends_on: condition: service_completed_successfully`).
- No separate `worker` service. **Decided (§12): in-process** — activity
  delivery (processing `delivery_queue`, phase 3) runs as a Tokio
  background task inside `backend` itself; revisit only if delivery
  volume ever warrants splitting it into its own process/image.
- `frontend` — React app; built as static assets and served either by
  `backend` (Axum `ServeDir` fallback) or its own lightweight `nginx`
  container. Simpler ops favors serving static assets straight from
  `backend`, avoiding a second container and CORS entirely.
- `caddy` or `nginx` (reverse proxy / TLS termination) — terminates TLS,
  proxies `backend`. Caddy preferred for automatic HTTPS via Let's Encrypt
  with minimal config.

`docker-compose.yml` (conceptual):

```yaml
services:
  postgres:
    image: postgres:16
    volumes: [pgdata:/var/lib/postgresql/data]
    environment: [...]
  rustfs:
    image: rustfs/rustfs:latest
    volumes: [rustfs_data:/data]
    environment: [RUSTFS_ROOT_USER, RUSTFS_ROOT_PASSWORD]
  backend:
    build: ./backend
    depends_on: [postgres, rustfs]
    environment: [DATABASE_URL, DOMAIN, OAUTH_..., S3_ENDPOINT_INTERNAL,
                  S3_ENDPOINT_PUBLIC, S3_BUCKET, S3_ACCESS_KEY, S3_SECRET_KEY, ...]
  proxy:
    image: caddy:2
    ports: ["80:80", "443:443"]
    depends_on: [backend, rustfs]
volumes:
  pgdata:
  rustfs_data:
```

Environment config via `.env` (not committed), read with `envy`/`dotenvy`
+ `serde`.

---

## 11. Frontend (React)

- Vite + TypeScript. **Decided: Apollo Client** for GraphQL — over
  `urql` (the spec's original lean toward lighter weight), picked
  instead for its more established ecosystem.
- Routing: **react-router** (v8, component API — `BrowserRouter`/
  `Routes`/`Route`, not the data-router, since there's no
  loader/action-worthy data layer yet). Implemented — see
  `frontend/src/App.tsx` and `frontend/src/routes/`.
- OAuth PKCE flow against the backend's own `/oauth/authorize` +
  `/oauth/token`.
- Pages: public post view, actor profile (posts + followers/following
  counts), home timeline (local + followed federated actors), post
  editor (Markdown), settings, login/consent screen, admin (user list —
  read-only; creation stays CLI-only in v1). Route shells for all of
  these already exist (placeholders, not yet wired to data).
- Post editor and avatar/header settings upload images by calling
  `requestImageUpload`, then `PUT`ing the file straight to the returned
  presigned RustFS URL from the browser (§5.2) — the GraphQL client never
  sends image bytes through `POST /graphql`.
- **Decided: client-only SPA** for public post/profile pages too (not
  server-rendered HTML or React SSR) — `/users/:username` and
  `/users/:username/:slug` are React routes. Accepted tradeoff: these
  pages have no content for crawlers/link-preview bots/no-JS clients
  until the JS bundle loads and fetches data; OpenGraph tags and
  Fediverse-preview crawlability are not solved by this and would need
  revisiting later if that matters. Practical implication for the
  backend: `routes/actor.rs`'s non-AS2-`Accept` branch (currently a
  placeholder HTML string) should ultimately serve the SPA's
  `index.html` shell instead, letting client-side routing take over —
  it does not need its own real HTML templating.

---

## 12. Decisions (formerly open)

All items below were open questions with a recommendation; each has now
been decided with the user directly. Kept as a single list (rather than
folded silently into the sections above) so the *alternatives* and
*why* stay visible — every decision here is also cross-referenced from
the section it affects.

1. **Delivery worker placement** — **Decided: in-process** Tokio task in
   `backend`, not a separate `worker` container (simpler Compose file,
   fine at blog-scale traffic; can split out later if needed). Affects
   §9/§10.
2. **Public page rendering** — **Decided: client-only SPA** for
   post/actor pages — *not* the originally-recommended plain
   server-rendered HTML, and not React SSR either. Accepted tradeoff:
   no content for crawlers/link-preview bots/no-JS clients until the JS
   bundle loads (OpenGraph/Fediverse-preview crawlability is unsolved
   for now). See §11 and §4.1 for what this changes; §3.2's actor-doc
   HTML branch now serves the SPA shell rather than server-rendering.
3. **`reader` role** — **Decided: deferred to v1.1.** Local accounts
   are all authors/admins for now; only remote Fediverse users
   follow/like/reply without owning a blog.
4. **Markdown → HTML sanitization** — **Decided: `ammonia`**, for
   federated `content` HTML (remote posts/replies arrive as HTML, not
   Markdown, and must be sanitized before storage/render).
5. **Tags/categories** — **Decided: deferred to v1.1** — easy to add
   later without touching the core post model. See §4.1.
6. **Full content vs. summary in feeds** — **Decided: full content** in
   `<content:encoded>`/`<content type="html">` (§4.2) — matches most
   feed readers and "read in reader" workflows.
7. **RustFS bucket visibility** — **Decided: public-read** bucket, not
   presigned GET URLs — blog images are meant to be public anyway, and
   it keeps §5.3 simple.
8. **Remote media caching** — **Decided: deferred to v1.1.** Remote
   actors' avatars/attachment images render via their own `url`s as-is
   (§5.4); no proxying/caching through RustFS yet.
9. **Image resizing/thumbnails** — **Decided: serve originals**, no
   resized variants — matches the "no media-heavy features" non-goal
   in §1; revisit if image sizes become a real problem.
10. **GraphQL client library** — **Decided: Apollo Client**, not `urql`
    (the original lean) — picked for its more established ecosystem.
    See §11.

---

## 13. Suggested phasing

- **Phase 0** — repo scaffold: Cargo workspace (`backend`, maybe a
  `activitypub` crate for AS2 types/HTTP Signatures kept separate from
  HTTP handlers), React app skeleton, Docker Compose (including
  `rustfs`), CI running `cargo test`/`cargo clippy` + frontend typecheck.
- **Phase 1** — local-only blog: users (via CLI), posts, image upload
  through RustFS (§5), public homepage and per-author pages, RSS/Atom
  feeds, GraphQL API, React reading/editing UI, OAuth provider, no
  federation yet.
- **Phase 2** — federation inbound: WebFinger, actor documents, inbox
  receiving Follow/Accept/Undo, signature verification.
- **Phase 3** — federation outbound: Create/Update/Delete delivery on
  publish/edit/delete, delivery queue + retries, outbox.
- **Phase 4** — replies/likes/announces both directions, timeline merging
  local + federated content.
- **Phase 5** — polish: NodeInfo, domain blocks, rate limiting, media
  attachments, notifications.
