//! Fill a database with a demo business: `cargo run --bin seed`
//!
//! Safe to run twice: it does nothing if the demo business already exists.

use anyhow::Result;
use chrono::{Datelike, Days, NaiveTime, TimeDelta, Utc};
use slotbook::availability::to_utc;
use slotbook::db;

const PASSWORD: &str = "demo password 123";

#[tokio::main]
async fn main() -> Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let pool = db::connect(&url, 2).await?;
    db::migrate(&pool).await?;

    if sqlx::query_scalar!("select id from organizations where slug = 'kape-barbers'")
        .fetch_optional(&pool)
        .await?
        .is_some()
    {
        println!("Demo data already there. Log in as ana@kape.example / {PASSWORD}");
        return Ok(());
    }

    let hash = {
        use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
        argon2::Argon2::default()
            .hash_password(PASSWORD.as_bytes(), &SaltString::generate(&mut OsRng))
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .to_string()
    };
    let mut tx = pool.begin().await?;
    let mut users = vec![];
    for (name, email) in [
        ("Ana Reyes", "ana@kape.example"),
        ("Ben Cruz", "ben@kape.example"),
    ] {
        let id = sqlx::query_scalar!(
            "insert into users (email, name, password_hash) values ($1, $2, $3) returning id",
            email,
            name,
            hash
        )
        .fetch_one(&mut *tx)
        .await?;
        users.push(id);
    }
    let org = sqlx::query_scalar!(
        "insert into organizations (name, slug, timezone)
         values ('Kape Barbers', 'kape-barbers', 'Pacific/Auckland') returning id"
    )
    .fetch_one(&mut *tx)
    .await?;
    for (user, role) in [(users[0], "owner"), (users[1], "staff")] {
        sqlx::query!(
            "insert into memberships (org_id, user_id, role) values ($1, $2, $3::text::member_role)",
            org,
            user,
            role
        )
        .execute(&mut *tx)
        .await?;
    }
    let mut services = vec![];
    for (name, minutes, cents) in [
        ("Haircut", 30, 3500),
        ("Skin fade", 45, 4500),
        ("Beard trim", 15, 2000),
    ] {
        let id = sqlx::query_scalar!(
            "insert into services (org_id, name, duration_minutes, price_cents) values ($1, $2, $3, $4) returning id",
            org,
            name,
            minutes,
            cents
        )
        .fetch_one(&mut *tx)
        .await?;
        services.push((id, minutes));
    }
    // Monday to Saturday, 9:00 to 17:00.
    for weekday in 0..6i16 {
        sqlx::query!(
            "insert into opening_hours (org_id, weekday, opens, closes) values ($1, $2, '09:00', '17:00')",
            org,
            weekday
        )
        .execute(&mut *tx)
        .await?;
    }
    // A few bookings over the next days.
    let tz = chrono_tz::Pacific::Auckland;
    let today = Utc::now().with_timezone(&tz).date_naive();
    let customers = [
        ("Carla Diaz", "carla@example.com"),
        ("Dan Ito", "dan@example.com"),
        ("Eve Ngata", "eve@example.com"),
    ];
    for (i, (name, email)) in customers.iter().enumerate() {
        let mut day = today + Days::new(i as u64 + 1);
        if day.weekday().num_days_from_monday() == 6 {
            day = day + Days::new(1);
        }
        let (service, minutes) = services[i];
        let start = to_utc(
            tz,
            day.and_time(NaiveTime::from_hms_opt(10 + i as u32, 0, 0).unwrap()),
        )
        .unwrap();
        sqlx::query!(
            "insert into bookings (org_id, service_id, staff_id, customer_name, customer_email, starts_at, ends_at)
             values ($1, $2, $3, $4, $5, $6, $7)",
            org,
            service,
            users[i % 2],
            name,
            email,
            start,
            start + TimeDelta::minutes(minutes.into())
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    println!("Seeded Kape Barbers: 2 team members, 3 services, 3 bookings.");
    println!(
        "Log in as ana@kape.example (owner) or ben@kape.example (staff), password: {PASSWORD}"
    );
    println!("Public booking page: /b/kape-barbers");
    Ok(())
}
