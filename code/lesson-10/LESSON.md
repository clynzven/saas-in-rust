# Lesson 10: A Testing Strategy That Finds Real Bugs

What each kind of test catches, an end-to-end journey, demo data, and what tests can't see.

This folder is the complete Slotbook project as it stands at the end of lesson 10
(Git tag `lesson-10` of this repository). It compiles and its tests pass.

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
 ...0290f0b6187f01f36c45e1dafd82ef2c446b58f5d7.json | 15 +++
 ...abdc9aefdf431bd9cee2742425ee9413a5e48dc615.json | 25 ++++
 ...744bea846d97bcb7b5eeba2fc25926666d5f4b18b7.json | 16 +++
 ...8d1fcb347499ddd7feb031b0bc8e19223a0ab22696.json | 20 ++++
 ...e3368927bb4175f286041304e2ac15305c182d49af.json | 20 ++++
 ...6c0706c8b42086a6cf1b59f13192b69052ae25d1ee.json | 20 ++++
 Cargo.toml                                         | 9 ++
 src/availability.rs                                | 21 ++++
 src/bin/seed.rs                                    | 133 +++++++++++++++++++++
 tests/journey.rs                                   | 110 +++++++++++++++++
 10 files changed, 389 insertions(+)
```

See it in full with `git diff lesson-09 lesson-10 -- code/slotbook-complete` from the repository root.
