# Lesson 6: Services and Opening Hours with HTMX

Forms that update in place with HTMX fragments, validation that people understand, and a bug only a browser could find.

This folder is the complete Slotbook project as it stands at the end of lesson 6
(Git tag `lesson-06` of this repository). It compiles and its tests pass.

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
 ...b12a565d218ce8543ef916f93d783bc73c43c00d61.json | 17 +
 ...a949bb8fb89eee943cce482c7c4626b4d8288cf483.json | 46 +++
 ...960e030606d321ad4e9658e5b56f81ca0c18da6d75.json | 17 +
 ...ad09a9d90a2392336144bb07de7cd9d204c01d38a1.json | 47 +++
 ...da67afe0d5b03735e75586ffd457056e4e2621b61f.json | 34 ++
 ...0b62a60a7225321cd9a0ea36d2819a76a3bf8572ba.json | 14 +
 src/catalog.rs                                     | 387 +++++++++++++++++++++
 src/lib.rs                                         | 13 +
 src/web.rs                                         | 5 +
 templates/base.html                                | 4 +-
 templates/hours.html                               | 24 ++
 templates/org.html                                 | 2 +
 templates/service_row.html                         | 14 +
 templates/services.html                            | 29 ++
 tests/catalog.rs                                   | 131 +++++++
 tests/common/mod.rs                                | 33 ++
 tests/orgs.rs                                      | 3 +-
 17 files changed, 817 insertions(+), 3 deletions(-)
```

See it in full with `git diff lesson-05 lesson-06 -- code/slotbook-complete` from the repository root.
