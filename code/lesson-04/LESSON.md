# Lesson 4: Accounts: Argon2 Passwords and Server-Side Sessions

Sign up, log in and log out, with Argon2 password hashing, hashed session tokens and safe cookies.

This folder is the complete Slotbook project as it stands at the end of lesson 4
(Git tag `lesson-04` of this repository). It compiles and its tests pass.

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
 ...d9617e9a961a7f61b3bc8f04254863f56c88314548.json | 28 ++++
 ...d51be925f13f3450ee5c8a3217525e7865de082e5e.json | 14 ++
 ...33058d7fd883387761540aff1373c8dbf9bdab97ec.json | 24 +++
 ...ce9940aa7d4797d92eefb835882bc7bf8d9dfee993.json | 34 +++++
 ...4725d6e9abbd2e8e20c2acfc2063e135b6101275ac.json | 34 +++++
 ...4d03576edea8b9f514aeedd7d92cda7142d889001c.json | 16 ++
 Cargo.lock                                         | 158 ++++++++++++++++++++
 Cargo.toml                                         | 7 +
 .../slotbook-complete/migrations/0002_sessions.sql | 10 ++
 src/accounts.rs                                    | 145 ++++++++++++++++++
 src/auth.rs                                        | 163 +++++++++++++++++++++
 src/dashboard.rs                                   | 40 +++++
 src/error.rs                                       | 6 +-
 src/forms.rs                                       | 65 ++++++++
 src/lib.rs                                         | 11 +-
 templates/app.html                                 | 6 +
 templates/base.html                                | 2 +-
 templates/dashboard.html                           | 15 ++
 templates/login.html                               | 12 ++
 templates/signup.html                              | 14 ++
 tests/accounts.rs                                  | 91 ++++++++++++
 tests/common/mod.rs                                | 60 ++++++++
 22 files changed, 952 insertions(+), 3 deletions(-)
```

See it in full with `git diff lesson-03 lesson-04 -- code/slotbook-complete` from the repository root.
