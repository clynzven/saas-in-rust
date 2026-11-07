# Lesson 7: The Availability Engine: Time Zones and Property Tests

Which times can a customer book? A pure function, correct across time zones and daylight saving, tested with properties.

This folder is the complete Slotbook project as it stands at the end of lesson 7
(Git tag `lesson-07` of this repository). It compiles and its tests pass.

## Run it

```bash
cp .env.example .env          # set DATABASE_URL
sqlx database create && sqlx migrate run
cargo test
cargo run
```

The tests create and drop their own temporary databases, so `DATABASE_URL` must point at a Postgres server where your user can create databases.
To build without a database, set `SQLX_OFFLINE=true` (the saved query data is in `.sqlx/`).

## What changed in this lesson

```
 Cargo.lock          | 203 +++++++++++++++++++++--
 Cargo.toml          | 1 +
 examples/slots.rs   | 50 ++++++
 src/availability.rs | 253 +++++++++++++++++++++++++++++
 src/lib.rs          | 1 +
 5 files changed, 498 insertions(+), 10 deletions(-)
```

See it in full with `git diff lesson-06 lesson-07 -- code/slotbook-complete` from the repository root.
