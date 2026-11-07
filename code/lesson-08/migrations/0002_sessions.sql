-- Server-side sessions. The cookie holds a random token; we store only its
-- SHA-256 hash, so a leaked database can't be used to log in as anyone.
create table sessions (
    token_hash bytea primary key,
    user_id    uuid not null references users on delete cascade,
    created_at timestamptz not null default now(),
    expires_at timestamptz not null
);
create index sessions_user_idx on sessions (user_id);
create index sessions_expires_idx on sessions (expires_at);
