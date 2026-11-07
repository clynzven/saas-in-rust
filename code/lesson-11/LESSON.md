# Lesson 11: Production Hardening

Graceful shutdown, timeouts, limits, request ids, panic safety, compression and housekeeping.

This folder is the complete Slotbook project as it stands at the end of lesson 11
(Git tag `lesson-11` of this repository). It compiles and its tests pass.

## Run it

```bash
cp .env.example .env          # set DATABASE_URL
sqlx database create && sqlx migrate run
cargo test
cargo run
```

The tests create and drop their own temporary databases, so `DATABASE_URL` must point at a Postgres server where your user can create databases.
To build without a database, set `SQLX_OFFLINE=true` (the saved query data is in `.sqlx/`).

## Fixed after a code review

A code and security review of the finished app found these; each fix lives in the lesson where the code first appears, with a test.

- The request log writes `/cancel/[secret]` instead of the customer's secret cancel token.
- Housekeeping also removes jobs that gave up, once they're a month old.

## What changed in this lesson

```
 ...7fbc1b4213015a270b48d3bff74e3e291942928be3.json | 14 +++
 ...b25fb103fbff730425d13d1acf573e05f2b432abfe.json | 12 +++
 Cargo.lock                                         | 117 +++++++++++++++++++++
 Cargo.toml                                         | 3 +-
 src/bin/seed.rs                                    | 2 +-
 src/config.rs                                      | 10 ++
 src/db.rs                                          | 4 +-
 src/jobs.rs                                        | 27 +++++
 src/lib.rs                                         | 80 ++++++++++----
 src/main.rs                                        | 37 ++++++-
 tests/common/mod.rs                                | 1 +
 tests/production.rs                                | 69 ++++++++++++
 12 files changed, 349 insertions(+), 27 deletions(-)
```

See it in full with `git diff lesson-10 lesson-11 -- code/slotbook-complete` from the repository root.
