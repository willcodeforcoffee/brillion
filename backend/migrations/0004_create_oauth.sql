create table oauth_applications (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    client_id text not null unique,
    client_secret_hash text not null,
    redirect_uris text[] not null,
    scopes text not null,
    created_at timestamptz not null default now()
);

create table oauth_authorization_codes (
    id uuid primary key default gen_random_uuid(),
    application_id uuid not null references oauth_applications(id) on delete cascade,
    user_id uuid not null references users(id) on delete cascade,
    code_hash text not null unique,
    code_challenge text not null,
    redirect_uri text not null,
    scopes text not null,
    expires_at timestamptz not null,
    used boolean not null default false,
    created_at timestamptz not null default now()
);

create table oauth_tokens (
    id uuid primary key default gen_random_uuid(),
    application_id uuid not null references oauth_applications(id) on delete cascade,
    user_id uuid not null references users(id) on delete cascade,
    token_hash text not null unique,
    refresh_token_hash text unique,
    scopes text not null,
    created_at timestamptz not null default now(),
    expires_at timestamptz,
    revoked_at timestamptz
);

create index oauth_tokens_user_id_idx on oauth_tokens (user_id);
