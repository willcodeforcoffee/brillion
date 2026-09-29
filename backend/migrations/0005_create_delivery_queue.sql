-- Outbound federation delivery queue (SPEC.md §3.4/§9) — a Tokio
-- background task inside `backend` drains this (§12: in-process, no
-- separate worker service).
--
-- `status` is only 'pending'/'delivered'/'failed'; there's no
-- 'in_progress' state. A claimed row is instead given a short lease by
-- pushing `next_attempt_at` forward (see `db::delivery_queue::claim_due`)
-- — if the worker crashes mid-delivery, the lease just expires and
-- another tick retries it, no separate crash-recovery path needed.
create table delivery_queue (
    id uuid primary key default gen_random_uuid(),
    actor_id uuid not null references actors(id) on delete cascade,
    inbox_url text not null,
    activity_json jsonb not null,
    status text not null default 'pending',
    attempts integer not null default 0,
    last_error text,
    next_attempt_at timestamptz not null default now(),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index delivery_queue_due_idx on delivery_queue (next_attempt_at) where status = 'pending';
