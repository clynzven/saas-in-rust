//! The schema enforces our most important business rule on its own.

use sqlx::PgPool;
use uuid::Uuid;

async fn setup(db: &PgPool) -> (Uuid, Uuid, Uuid) {
    let user: Uuid = sqlx::query_scalar(
        "insert into users (email, name, password_hash) values ('ana@example.com', 'Ana', 'x') returning id",
    )
    .fetch_one(db).await.unwrap();
    let org: Uuid = sqlx::query_scalar(
        "insert into organizations (name, slug, timezone) values ('Kape Barbers', 'kape-barbers', 'Pacific/Auckland') returning id",
    )
    .fetch_one(db).await.unwrap();
    let service: Uuid = sqlx::query_scalar(
        "insert into services (org_id, name, duration_minutes, price_cents) values ($1, 'Haircut', 30, 3500) returning id",
    )
    .bind(org).fetch_one(db).await.unwrap();
    (user, org, service)
}

async fn book(db: &PgPool, ids: (Uuid, Uuid, Uuid), start: &str, end: &str) -> Result<(), sqlx::Error> {
    let (staff, org, service) = ids;
    sqlx::query(
        "insert into bookings (org_id, service_id, staff_id, customer_name, customer_email, starts_at, ends_at)
         values ($1, $2, $3, 'Ben', 'ben@example.com', $4::timestamptz, $5::timestamptz)",
    )
    .bind(org).bind(service).bind(staff).bind(start).bind(end)
    .execute(db).await.map(|_| ())
}

#[sqlx::test]
async fn overlapping_bookings_are_rejected(db: PgPool) {
    let ids = setup(&db).await;
    book(&db, ids, "2026-12-01 09:00Z", "2026-12-01 09:30Z").await.unwrap();

    let err = book(&db, ids, "2026-12-01 09:15Z", "2026-12-01 09:45Z").await.unwrap_err();
    let code = err.as_database_error().and_then(|e| e.constraint());
    assert_eq!(code, Some("no_double_booking"));
}

#[sqlx::test]
async fn back_to_back_bookings_are_fine(db: PgPool) {
    let ids = setup(&db).await;
    book(&db, ids, "2026-12-01 09:00Z", "2026-12-01 09:30Z").await.unwrap();
    // tstzrange is [start, end): ending at 09:30 doesn't overlap starting at 09:30.
    book(&db, ids, "2026-12-01 09:30Z", "2026-12-01 10:00Z").await.unwrap();
}
