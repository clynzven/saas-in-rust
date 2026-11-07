# Lesson 5: Multi-Tenancy, Roles and Security Layers

Businesses, owner and staff roles, an extractor that enforces tenant isolation, and three security layers.

This folder is the complete Slotbook project as it stands at the end of lesson 5
(Git tag `lesson-05` of this repository). It compiles and its tests pass.

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

- Wrong passwords are counted per network **and** email, so someone guessing your password elsewhere can't lock you out; each network also has a cap on logins and sign-ups (`ClientIp`, `ip_limiter`).
- The rate limiter's memory is bounded: long keys are cut and a flood of new keys is refused instead of growing without limit.
- Adding team members is rate limited, because the answer reveals whether an email has an account.
- Behind a proxy, set `TRUSTED_IP_HEADER` (see `.env.example`).

## What changed in this lesson

```
 .env.example                                       | 2 +
 ...144e06f712a859d53868e5b634de94b1290d4410e2.json | 50 +++
 ...ec8fc8a02a10f73e1eb2442115a87c81c90f6d9a9e.json | 15 +
 ...a93e167a07fc6a1434887b7d86c7e8746adee11b57.json | 22 ++
 ...be30d146cecff6efd6d81ff063779b2d21de99c89c.json | 15 +
 ...ea91821547c1be144ee7f1129066d64e4589f87b1e.json | 24 ++
 ...03c16d09b9fcbec2628ec874a78719269a6611ec99.json | 57 ++++
 ...bdf7b4128567ce79f24fef5e67e4845bc26fa73bde.json | 15 +
 ...c343625a9b067343191d802053c11103c7ba118565.json | 22 ++
 src/accounts.rs                                    | 24 ++
 src/config.rs                                      | 6 +
 src/lib.rs                                         | 16 +
 src/main.rs                                        | 13 +-
 src/orgs.rs                                        | 358 +++++++++++++++++++++
 src/security.rs                                    | 182 +++++++++++
 src/state.rs                                       | 19 +-
 static/app.css                                     | 2 +
 templates/app.html                                 | 2 +-
 templates/base.html                                | 1 +
 templates/dashboard.html                           | 3 +
 templates/org.html                                 | 6 +
 templates/org_home.html                            | 6 +
 templates/org_new.html                             | 21 ++
 templates/team.html                                | 26 ++
 tests/common/mod.rs                                | 8 +-
 tests/orgs.rs                                      | 222 +++++++++++++
 26 files changed, 1122 insertions(+), 15 deletions(-)
```

See it in full with `git diff lesson-04 lesson-05 -- code/slotbook-complete` from the repository root.
