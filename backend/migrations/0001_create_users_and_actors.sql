create extension if not exists pgcrypto;

create type user_role as enum ('admin', 'author');

create table users (
    id uuid primary key default gen_random_uuid(),
    email text not null unique,
    password_hash text not null,
    role user_role not null default 'author',
    created_at timestamptz not null default now()
);

-- Local actors have user_id set and is_local = true. Remote actors
-- (cached copies of federated profiles, added from phase 2 onward) have
-- user_id null and is_local = false.
create table actors (
    id uuid primary key default gen_random_uuid(),
    user_id uuid references users(id) on delete cascade,
    preferred_username text not null,
    domain text not null,
    display_name text not null default '',
    bio text not null default '',
    avatar_url text,
    header_url text,
    inbox_url text not null,
    outbox_url text not null,
    followers_url text not null,
    following_url text not null,
    public_key_pem text not null,
    private_key_pem text,
    is_local boolean not null default true,
    created_at timestamptz not null default now(),
    unique (preferred_username, domain)
);

create index actors_user_id_idx on actors (user_id) where user_id is not null;
