# Lesson 12: Ship It: Docker, CI/CD and Deployment

A small, secure Docker image, CI that tests every push, continuous deployment, and a go-live checklist.

This folder is the complete Slotbook project as it stands at the end of lesson 12
(Git tag `lesson-12` of this repository). It compiles and its tests pass.

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

- `fly.toml` sets `TRUSTED_IP_HEADER = "Fly-Client-IP"`, so rate limits see each visitor, not Fly's proxy; the README and `docs/deploy.md` explain the setting for Caddy.

## What changed in this lesson

```
 .dockerignore            | 5 ++
 .github/workflows/ci.yml | 81 ++++++++++++++++++
 Dockerfile               | 29 +++++++
 LICENSE                  | 21 +++++
 README.md                | 104 ++++++++++++++++++++++++
 compose.yaml             | 37 +++++++++
 docs/deploy.md           | 63 ++++++++++++++
 fly.toml                 | 31 +++++++
 static/app.css           | 2 +
 templates/services.html  | 2 +-
 10 files changed, 374 insertions(+), 1 deletion(-)
```

See it in full with `git diff lesson-11 lesson-12 -- code/slotbook-complete` from the repository root.
