# Lesson 8: Public Booking and Concurrency You Can Trust

The public booking flow, a booking insert that's safe under concurrency, and the deadlock my first version had.

This folder is the complete Slotbook project as it stands at the end of lesson 8
(Git tag `lesson-08` of this repository). It compiles and its tests pass.

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

- The "You're booked" page shows the email masked (`b***@example.com`): anyone with the link could see it before.
- Staff can't cancel bookings that already started, and the list hides the Cancel button for them.

## What changed in this lesson

```
 ...d5b941faa057dee82fdfd6cf55fd5ca5e68d631035.json | 40 ++
 ...7757d44ae133982fb9ce6749e0e31a91f3a66770bd.json | 22 +
 ...aa63328c19ad7655719911871c02634ccdff1d5171.json | 36 ++
 ...d8a0d68e10bc4e56082ca282f38e35c454d78fbc1b.json | 40 ++
 ...e0120bedf3a5bca86a7c6e1a66c688a67e5284223e.json | 29 ++
 ...92e6bd927d00f7d55bc58c1122fbe065332e5e8e12.json | 41 ++
 ...c8b7250dfc3ba4e566ee30c02305c1a44b04de6791.json | 64 +++
 ...819cebc7a4c6ab1c32eb0ccb47664ee447199fdc63.json | 65 +++
 ...a40685e1e13a71904aeeafe15e4c2a49dae9b3b85a.json | 35 ++
 ...26bec7db9d71c2c0965a4e80754f0e01035dff9056.json | 22 +
 ...b3102d9010bece8e0e94d0a52e0d238d8003823621.json | 28 ++
 Cargo.lock                                         | 29 ++
 Cargo.toml                                         | 1 +
 src/booking.rs                                     | 534 +++++++++++++++++++++
 src/error.rs                                       | 5 +
 src/lib.rs                                         | 13 +
 templates/booking_row.html                         | 14 +
 templates/bookings.html                            | 12 +
 templates/org.html                                 | 1 +
 templates/org_home.html                            | 4 +
 templates/public.html                              | 2 +
 templates/public_book.html                         | 15 +
 .../slotbook-complete/templates/public_booked.html | 8 +
 templates/public_org.html                          | 16 +
 .../templates/public_service.html                  | 28 ++
 tests/booking.rs                                   | 296 ++++++++++++
 26 files changed, 1400 insertions(+)
```

See it in full with `git diff lesson-07 lesson-08 -- code/slotbook-complete` from the repository root.
