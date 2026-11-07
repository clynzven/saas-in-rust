# Lesson 2: The Database: Schema, Migrations and Constraints

Design the schema for a multi-business product, run migrations, and let Postgres enforce the no-double-booking rule.

This folder is the complete Slotbook project as it stands at the end of lesson 2
(Git tag `lesson-02` of this repository). It compiles and its tests pass.

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
 .env.example                                       | 1 +
 ...93474fc26ae9c855f65f5b73b0a5038e55f8c9c57f.json | 20 +
 Cargo.lock                                         | 1748 +++++++++++++++++++-
 Cargo.toml                                         | 4 +
 migrations/0001_init.sql                           | 75 +
 src/config.rs                                      | 5 +-
 src/db.rs                                          | 23 +
 src/lib.rs                                         | 22 +-
 src/main.rs                                        | 10 +-
 src/state.rs                                       | 13 +
 tests/health.rs                                    | 30 +-
 tests/schema.rs                                    | 48 +
 12 files changed, 1954 insertions(+), 45 deletions(-)
```

See it in full with `git diff lesson-01 lesson-02 -- code/slotbook-complete` from the repository root.
