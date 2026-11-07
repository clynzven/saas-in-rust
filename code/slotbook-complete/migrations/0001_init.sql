-- Slotbook schema, version 1.
-- Every table that belongs to a business carries org_id: that's our tenant key.

create extension if not exists btree_gist;

create table users (
    id            uuid primary key default gen_random_uuid(),
    email         text not null,
    name          text not null,
    password_hash text not null,
    created_at    timestamptz not null default now()
);
-- Emails are unique regardless of case: Ana@x.com and ana@x.com are one person.
create unique index users_email_key on users (lower(email));

create table organizations (
    id         uuid primary key default gen_random_uuid(),
    name       text not null,
    slug       text not null unique check (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    timezone   text not null,
    created_at timestamptz not null default now()
);

create type member_role as enum ('owner', 'staff');

create table memberships (
    org_id     uuid not null references organizations on delete cascade,
    user_id    uuid not null references users on delete cascade,
    role       member_role not null,
    created_at timestamptz not null default now(),
    primary key (org_id, user_id)
);

create table services (
    id               uuid primary key default gen_random_uuid(),
    org_id           uuid not null references organizations on delete cascade,
    name             text not null check (length(name) between 1 and 80),
    duration_minutes integer not null check (duration_minutes between 5 and 480),
    price_cents      integer not null check (price_cents >= 0),
    active           boolean not null default true,
    created_at       timestamptz not null default now()
);
create index services_org_idx on services (org_id);

-- Opening hours in the business's own time zone. 0 = Monday ... 6 = Sunday.
create table opening_hours (
    org_id  uuid not null references organizations on delete cascade,
    weekday smallint not null check (weekday between 0 and 6),
    opens   time not null,
    closes  time not null,
    check (opens < closes),
    primary key (org_id, weekday)
);

create table bookings (
    id             uuid primary key default gen_random_uuid(),
    org_id         uuid not null references organizations on delete cascade,
    service_id     uuid not null references services,
    staff_id       uuid not null references users,
    customer_name  text not null,
    customer_email text not null,
    starts_at      timestamptz not null,
    ends_at        timestamptz not null,
    status         text not null default 'confirmed'
                   check (status in ('confirmed', 'cancelled')),
    created_at     timestamptz not null default now(),
    check (starts_at < ends_at),
    -- The database itself refuses two confirmed bookings for the same
    -- staff member that overlap in time, even under heavy concurrency.
    constraint no_double_booking exclude using gist (
        staff_id with =,
        tstzrange(starts_at, ends_at) with &&
    ) where (status = 'confirmed')
);
create index bookings_org_start_idx on bookings (org_id, starts_at);
