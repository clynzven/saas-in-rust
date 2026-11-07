//! One test that tells the whole product story through HTTP, the way a real
//! owner, staff member and customer would use it. If this passes, the main
//! path works end to end.

mod common;

use axum::http::StatusCode;
use common::{TestApp, local_day, three_days_out_at_10};
use sqlx::PgPool;

#[sqlx::test]
async fn the_whole_story(db: PgPool) {
    let app = TestApp::new(db.clone());

    // 1. Ana signs up and creates her barbershop.
    let ana = app.signup("Ana Reyes", "ana@kape.example").await;
    let res = app
        .post_form(
            "/orgs",
            &[
                ("name", "Kape Barbers"),
                ("slug", "kape-barbers"),
                ("timezone", "Pacific/Auckland"),
            ],
            Some(&ana),
        )
        .await;
    assert_eq!(res.location(), "/app/kape-barbers");

    // 2. She adds a service and opens Monday to Sunday, 9 to 5.
    app.htmx_post(
        "/app/kape-barbers/services",
        &[
            ("name", "Haircut"),
            ("duration_minutes", "30"),
            ("price", "35"),
        ],
        &ana,
    )
    .await;
    let mut hours = vec![];
    for d in 0..7 {
        hours.extend([
            (format!("open_{d}"), "on".to_string()),
            (format!("opens_{d}"), "09:00".to_string()),
            (format!("closes_{d}"), "17:00".to_string()),
        ]);
    }
    let hours: Vec<(&str, &str)> = hours
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let saved = app.htmx_post("/app/kape-barbers/hours", &hours, &ana).await;
    assert!(saved.body.contains("Opening hours saved"));

    // 3. Ben joins as staff.
    let ben = app.signup("Ben Cruz", "ben@kape.example").await;
    app.post_form(
        "/app/kape-barbers/team",
        &[("email", "ben@kape.example")],
        Some(&ana),
    )
    .await;

    // 4. A customer finds the page, picks a day and books 10:00.
    let page = app.get("/b/kape-barbers").await;
    let service = page.body.split("/b/kape-barbers/").nth(1).unwrap()[..36].to_string();
    let start = three_days_out_at_10();
    let times = app
        .get(&format!(
            "/b/kape-barbers/{service}?date={}",
            local_day(&start)
        ))
        .await;
    assert!(times.body.contains("10:00 am"));
    let booked = app
        .post_form(
            &format!("/b/kape-barbers/{service}/book"),
            &[
                ("start", &start),
                ("name", "Carla Diaz"),
                ("email", "carla@example.com"),
            ],
            None,
        )
        .await;
    assert_eq!(booked.status, StatusCode::SEE_OTHER);

    // 5. The worker sends her a confirmation with a cancel link.
    app.run_jobs().await;
    let email = app.emails().pop().unwrap();
    assert_eq!(email.to, "carla@example.com");

    // 6. Ben (staff) sees the booking on the team's list.
    let list = app.get_as("/app/kape-barbers/bookings", &ben).await;
    assert!(list.body.contains("Carla Diaz"));

    // 7. Carla can't make it and cancels from the email.
    let link = email.text[email.text.find("/cancel/").unwrap()..]
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    let cancelled = app.post_form(&link, &[], None).await;
    assert!(cancelled.body.contains("Your booking is cancelled"));

    // 8. The list shows it as cancelled.
    let list = app.get_as("/app/kape-barbers/bookings", &ana).await;
    assert!(list.body.contains("pill cancelled"));
}
