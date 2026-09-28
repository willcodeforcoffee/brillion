create table posts (
    id uuid primary key default gen_random_uuid(),
    actor_id uuid not null references actors(id) on delete cascade,
    slug text not null,
    title text not null,
    summary text,
    body_markdown text not null,
    body_html text not null,
    status text not null default 'draft',
    ap_object_id text,
    published_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (actor_id, slug)
);

create index posts_actor_id_idx on posts (actor_id);
create index posts_published_idx on posts (published_at desc) where status = 'published';

create table media_attachments (
    id uuid primary key default gen_random_uuid(),
    post_id uuid references posts(id) on delete cascade,
    object_key text not null,
    url text not null,
    media_type text not null,
    byte_size bigint not null,
    width integer,
    height integer,
    alt_text text,
    created_at timestamptz not null default now()
);

create index media_attachments_post_id_idx on media_attachments (post_id) where post_id is not null;
