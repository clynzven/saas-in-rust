# Slotbook

Online booking for small businesses, built in Rust. A barbershop, studio or clinic
sets its services and opening hours, shares one link, and customers book a time
that is actually free. Staff see a tidy list of bookings, and customers get a
confirmation email with a link to cancel.

It is the project built step by step in the course **Build a SaaS in Rust**.

## What's inside

| Area | How |
|---|---|
| Web | Axum 0.8, server-rendered Askama templates, HTMX for in-place updates (works without JavaScript too) |
| Data | PostgreSQL with SQLx compile-time checked queries and migrations |
| Accounts | Argon2id passwords, server-side sessions (only token hashes stored), HttpOnly SameSite cookies |
| Multi-tenancy | Every business row carries `org_id`; an `Org` extractor checks membership on every team page; non-members get 404 |
| Roles | Owners manage services, hours and the team; staff can view and handle bookings |
| Bookings | A pure availability engine (time zones and daylight saving), and a Postgres exclusion constraint so no staff member is ever double-booked, even under concurrency |
| Email & jobs | Transactional outbox in Postgres, a worker using `FOR UPDATE SKIP LOCKED`, retries with exponential backoff, SMTP via lettre |
| Security | Origin checks on every state-changing request, CSP and other security headers, login rate limiting, request size limit |
| Operations | JSON logs with request ids, `/health` and `/ready`, timeouts, panic recovery, compression, graceful shutdown on SIGTERM, housekeeping |
| Shipping | Multi-stage Dockerfile (distroless, non-root), Docker Compose with Postgres and Mailpit, GitHub Actions CI/CD, Fly.io config |

## Quick start

You need Rust 1.85 or newer and PostgreSQL 14 or newer (or Docker).

```bash
# Everything in containers:
docker compose up --build
# App: http://localhost:8080   Emails: http://localhost:8025

# Or run Rust locally against your own Postgres:
cp .env.example .env            # then edit DATABASE_URL
cargo install sqlx-cli --no-default-features --features postgres,rustls
sqlx database create && sqlx migrate run
cargo run --bin seed            # demo business: ana@kape.example / "demo password 123"
cargo run                       # http://localhost:3000
```

## Tests

```bash
cargo test
```

There are 67 tests: unit tests, property tests for the availability engine, a doctest,
and HTTP-level integration tests that each run against a fresh database
(`#[sqlx::test]`). They include a test where 20 customers try to book the same slot
at once and exactly one succeeds, and an end-to-end journey from sign-up to cancellation.
They need `DATABASE_URL` to point at a Postgres server where they can create databases.

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `DATABASE_URL` | (required) | `postgres://user:password@host/db` |
| `DATABASE_MAX_CONNECTIONS` | 10 | Pool size per instance |
| `BIND_ADDR` | 127.0.0.1:3000 | Use 0.0.0.0:8080 in containers |
| `BASE_URL` | http://localhost:3000 | Public URL, used in email links; `https://` turns on secure cookies |
| `SMTP_URL` | (unset: emails are logged) | e.g. `smtps://user:pass@smtp.example.com`; required when `BASE_URL` is `https://` |
| `TRUSTED_IP_HEADER` | (unset) | Behind a proxy, the header with the visitor's IP: `Fly-Client-IP` on Fly.io, `X-Forwarded-For` behind Caddy. Leave unset without a proxy |
| `MAIL_FROM` | Slotbook <bookings@localhost> | Sender address |
| `LOG_FORMAT` | pretty | `json` in production |
| `STATIC_DIR` | ./static | Where CSS and JavaScript are served from |

## Project layout

```
src/
  main.rs          start-up, worker, graceful shutdown
  lib.rs           the router and middleware stack
  config.rs db.rs state.rs error.rs web.rs forms.rs
  auth.rs          passwords, sessions, CurrentUser extractor
  security.rs      origin check, security headers, rate limiter
  accounts.rs      sign up, log in, log out
  orgs.rs          businesses, roles, the Org extractor, team
  catalog.rs       services and opening hours
  availability.rs  the pure slot engine
  booking.rs       public booking flow and staff bookings list
  cancel.rs        customer cancellation links
  jobs.rs mail.rs  outbox queue, worker, email
  bin/seed.rs      demo data
migrations/        SQL, applied at start-up
templates/         Askama templates
tests/             integration tests
```

## Deploying

See [docs/deploy.md](docs/deploy.md) for Fly.io and for any Linux server with Docker,
plus backups and a go-live checklist.

## Limits and next steps

- One opening-hours range per weekday (no lunch breaks yet) and no holidays or time off.
- Staff are added by the email they signed up with; there are no email invitations yet.
- The login rate limiter is in memory: with several instances, move it to Postgres or Redis.
- No payments, reminders or calendar sync yet. These are good next features.

## License

MIT
