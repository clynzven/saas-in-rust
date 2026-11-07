-- A durable job queue in Postgres. Jobs are inserted in the same transaction
-- as the change that causes them (the "outbox" pattern), so a booking can
-- never exist without its confirmation email being queued, or vice versa.
create table jobs (
    id         bigserial primary key,
    kind       text not null,
    payload    jsonb not null,
    run_at     timestamptz not null default now(),
    attempts   integer not null default 0,
    last_error text,
    done_at    timestamptz,
    created_at timestamptz not null default now()
);
create index jobs_due_idx on jobs (run_at) where done_at is null;

-- Customers cancel with a secret link. As with sessions, we keep only a hash.
alter table bookings add column cancel_token_hash bytea;
create unique index bookings_cancel_token_idx on bookings (cancel_token_hash);
