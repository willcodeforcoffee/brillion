-- The actor's own AS2 `id` URL. For local actors this is
-- `{PUBLIC_BASE_URL}/users/{preferred_username}` (backfilled below for
-- any rows created before this column existed); for remote actors it's
-- whatever they report in their own Person document — never assumed
-- from `domain`/`preferred_username`, since that pattern isn't
-- guaranteed across Fediverse software.
alter table actors add column ap_id text;
update actors set ap_id = 'https://' || domain || '/users/' || preferred_username
    where ap_id is null;
alter table actors alter column ap_id set not null;
alter table actors add constraint actors_ap_id_key unique (ap_id);

create table follows (
    id uuid primary key default gen_random_uuid(),
    follower_actor_id uuid not null references actors(id) on delete cascade,
    followee_actor_id uuid not null references actors(id) on delete cascade,
    state text not null default 'pending',
    ap_activity_id text,
    created_at timestamptz not null default now(),
    unique (follower_actor_id, followee_actor_id)
);

create index follows_follower_idx on follows (follower_actor_id);
create index follows_followee_idx on follows (followee_actor_id);

create table domain_blocks (
    id uuid primary key default gen_random_uuid(),
    domain text not null unique,
    reason text,
    created_at timestamptz not null default now()
);

-- Inbound/outbound activity log, keyed by the activity's own AS2 `id`
-- for idempotency (SPEC.md §3.3: "dedupes by activity id").
create table activities_log (
    id uuid primary key default gen_random_uuid(),
    direction text not null,
    ap_id text not null unique,
    activity_json jsonb not null,
    actor_id uuid references actors(id) on delete set null,
    processed_at timestamptz,
    error text,
    created_at timestamptz not null default now()
);
