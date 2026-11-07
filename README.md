# Build a SaaS in Rust: source code

The code for the course and book *Build a SaaS in Rust* from ClynzVen Code
Academy. Released under the MIT License.

## What's here

| Folder | Contents |
|---|---|
| `code/lesson-01` ... `code/lesson-12` | The whole project at the end of each lesson. Each folder compiles, its tests pass, and `LESSON.md` says what changed and how to run it |
| `code/slotbook-complete` | The finished project. The repository history has one commit per lesson, tagged `lesson-01` to `lesson-12` |

## Requirements

- Rust 1.85 or newer (tested with 1.97)
- PostgreSQL 14 or newer, or Docker
- `cargo install sqlx-cli --no-default-features --features postgres,rustls`

## Quick start with the finished app

```bash
cd code/slotbook-complete
docker compose up --build          # app on :8080, caught emails on :8025
# or, with your own Postgres:
cp .env.example .env               # set DATABASE_URL
sqlx database create && sqlx migrate run
cargo run --bin seed               # demo login: ana@kape.example / demo password 123
cargo run                          # http://localhost:3000
cargo test                         # 67 tests
```

Tip: give each lesson folder its own database name in `.env`. A database migrated by
a later lesson has migrations an earlier lesson doesn't know about.
