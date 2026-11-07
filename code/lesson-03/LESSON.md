# Lesson 3: Pages: Askama Templates, HTMX and One Error Type

Server-rendered HTML with Askama, a tiny stylesheet, HTMX, and one error type that every handler shares.

This folder is the complete Slotbook project as it stands at the end of lesson 3
(Git tag `lesson-03` of this repository). It compiles and its tests pass.

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
 Cargo.lock               | 130 ++++++++++++++++++++++++
 Cargo.toml               | 4 +-
 src/config.rs            | 7 +-
 src/error.rs             | 71 +++++++++++++
 src/lib.rs               | 24 ++++-
 src/main.rs              | 5 +-
 src/pages.rs             | 19 ++++
 src/web.rs               | 11 ++
 static/app.css           | 45 ++++++++
 static/htmx-2.0.4.min.js | 1 +
 templates/base.html      | 22 ++++
 templates/error.html     | 7 ++
 templates/home.html      | 11 ++
 tests/common/mod.rs      | 72 +++++++++++++
 tests/health.rs          | 30 ++----
 tests/pages.rs           | 20 ++++
 tests/schema.rs          | 23 ++++-
 17 files changed, 470 insertions(+), 32 deletions(-)
```

See it in full with `git diff lesson-02 lesson-03 -- code/slotbook-complete` from the repository root.
