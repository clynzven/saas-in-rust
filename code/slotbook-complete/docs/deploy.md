# Deploying Slotbook

Slotbook is one binary plus a `static/` folder, and it needs PostgreSQL.
Migrations run automatically at start-up (SQLx takes a lock, so several
instances starting at once is safe).

## Option 1: Fly.io

```bash
fly launch --no-deploy                 # keep the included fly.toml
fly postgres create --name slotbook-db # or use any managed Postgres (Neon, Supabase, RDS)
fly postgres attach slotbook-db        # sets DATABASE_URL as a secret
fly secrets set SMTP_URL='smtps://USER:PASSWORD@smtp.example.com' \
                MAIL_FROM='Slotbook <bookings@yourdomain.com>'
# Edit BASE_URL in fly.toml to your real URL, then:
fly deploy
```

Fly sends SIGTERM and waits up to `kill_timeout` (30 s), which matches the app's
graceful shutdown. The `/ready` check keeps traffic away until the database is reachable.

To deploy from GitHub Actions, add the repository secret `FLY_API_TOKEN`
(`fly tokens create deploy`) and the variable `FLY_APP`. The `deploy` job runs only
after the tests pass and the image builds.

## Option 2: any Linux server with Docker

1. Install Docker and point a domain at the server.
2. Copy `compose.yaml`, remove the `mail` service, and set real values for
   `BASE_URL` (https), `SMTP_URL`, `MAIL_FROM` and a strong Postgres password.
   Set `TRUSTED_IP_HEADER=X-Forwarded-For` so rate limits see each visitor's IP, not Caddy's.
   Better still, use the image CI pushes: `image: ghcr.io/<you>/slotbook:latest`.
3. Put Caddy in front for automatic HTTPS. A whole `Caddyfile`:

   ```
   book.example.com {
       reverse_proxy localhost:8080
   }
   ```
4. `docker compose up -d`. Updates: `docker compose pull && docker compose up -d`.

## Backups

Bookings are your customers' data. Back them up and test a restore.

```bash
# Nightly, e.g. from cron. Keep copies off the server.
docker compose exec -T db pg_dump -U postgres -Fc slotbook > slotbook-$(date +%F).dump
# Restore into an empty database:
pg_restore -U postgres -d slotbook --clean slotbook-2026-11-07.dump
```

Managed Postgres services also offer point-in-time recovery: turn it on.

## Go-live checklist

- [ ] `BASE_URL` starts with `https://` (this turns on Secure cookies)
- [ ] `SMTP_URL` set, and a test booking's email arrives (check spam; set up SPF and DKIM for your sending domain)
- [ ] `LOG_FORMAT=json` and logs are collected somewhere you can search by `x-request-id`
- [ ] `/ready` is the health check
- [ ] Backups run, and you have restored one
- [ ] Total `DATABASE_MAX_CONNECTIONS` across instances is below the database's limit
- [ ] Failed jobs are checked: `select id, kind, attempts, last_error from jobs where done_at is null and attempts >= 6;`
