# Lesson 9: Emails and Background Jobs with a Postgres Queue

A durable job queue in Postgres, the transactional outbox pattern, retries with backoff, and cancel links.

This folder is the complete Slotbook project as it stands at the end of lesson 9
(Git tag `lesson-09` of this repository). It compiles and its tests pass.

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

- A confirmation job that gives up drops the raw cancel token from its payload.
- `BASE_URL=https://...` without `SMTP_URL` stops at startup: logging emails would put cancel links in production logs.
- Blocked cross-origin requests log `/cancel/[secret]` instead of the token (`loggable_path`).

## What changed in this lesson

```
 ...83a78e973d1adb5afef7170489044066cb9486a7a.json} | 7 +-
 ...a2629fb07ac8372e57973894df0394e83ee28bcc1e.json | 34 ++++
 ...89de95843f28c5d785eef0d93c7e3e7b6a03b132c1.json | 14 ++
 ...e86a2d8fb314d51e864db29f3559acbbade51cf704.json | 17 ++
 ...64ff5f80895d97e88472cc5df8a729524f6684855.json} | 4 +-
 ...8afe52165b2625d41d266819ae3157671f4972b7ec.json | 58 ++++++
 ...58dccf0cdb61feec427f33c942042dab2db32a69e7.json | 15 ++
 ...7c411e27943abd18307f8b40d52e2a2661394c0d6b.json | 58 ++++++
 ...e1e277e7803c19c17d228c6bc69e91baef40161c58.json | 14 ++
 Cargo.lock                                         | 108 +++++++++-
 Cargo.toml                                         | 4 +-
 migrations/0003_jobs.sql                           | 18 ++
 src/booking.rs                                     | 26 ++-
 src/cancel.rs                                      | 110 ++++++++++
 src/config.rs                                      | 29 +++
 src/jobs.rs                                        | 224 +++++++++++++++++++++
 src/lib.rs                                         | 4 +
 src/mail.rs                                        | 69 +++++++
 src/main.rs                                        | 20 +-
 src/security.rs                                    | 21 +-
 src/state.rs                                       | 5 +-
 templates/cancel.html                              | 20 ++
 tests/booking.rs                                   | 70 +------
 tests/common/mod.rs                                | 69 ++++++-
 tests/emails.rs                                    | 147 ++++++++++++++
 25 files changed, 1085 insertions(+), 80 deletions(-)
```

See it in full with `git diff lesson-08 lesson-09 -- code/slotbook-complete` from the repository root.
