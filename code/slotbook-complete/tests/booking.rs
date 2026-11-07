mod common;

use axum::http::StatusCode;
use common::{TestApp, local_day, three_days_out_at_10};
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test]
async fn public_page_lists_active_services(db: PgPool) {
    let app = TestApp::new(db);
    common::open_business(&app).await;
    let page = app.get("/b/kape-barbers").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("Haircut") && page.body.contains("30 min · $35.00"));
}

#[sqlx::test]
async fn customer_books_a_slot(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (_, service) = common::open_business(&app).await;
    let start = three_days_out_at_10();

    let res = app
        .post_form(
            &format!("/b/kape-barbers/{service}/book"),
            &[
                ("start", &start),
                ("name", "Ben Cruz"),
                ("email", "ben@example.com"),
            ],
            None,
        )
        .await;
    assert_eq!(res.status, StatusCode::SEE_OTHER, "{}", res.body);
    let confirmation = app.get(res.location()).await;
    assert!(confirmation.body.contains("You're booked in"));
    assert!(confirmation.body.contains("10:00 am"));

    // The same time is gone from the list now (only one staff member).
    let day = local_day(&start);
    let times = app
        .get(&format!("/b/kape-barbers/{service}?date={day}"))
        .await;
    assert!(
        times.body.contains("9:30 am") && !times.body.contains(">10:00 am<"),
        "{}",
        times.body
    );
}

#[sqlx::test]
async fn htmx_date_change_returns_only_the_times(db: PgPool) {
    let app = TestApp::new(db);
    let (_, service) = common::open_business(&app).await;
    let req = axum::http::Request::get(format!("/b/kape-barbers/{service}"))
        .header("hx-request", "true")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.send(req).await;
    assert!(res.body.trim_start().starts_with("<div id=\"slots\""));
}

#[sqlx::test]
async fn twenty_customers_one_slot_one_winner(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (_, service) = common::open_business(&app).await;
    let start = three_days_out_at_10();
    let path = format!("/b/kape-barbers/{service}/book");

    // Fire 20 bookings for the same time at once.
    let attempts = (0..20).map(|i| {
        let (app, path, start) = (&app, &path, &start);
        async move {
            let email = format!("c{i}@example.com");
            app.post_form(
                path,
                &[("start", start), ("name", "Customer"), ("email", &email)],
                None,
            )
            .await
            .status
        }
    });
    let results = futures::future::join_all(attempts).await;

    let won = results
        .iter()
        .filter(|s| **s == StatusCode::SEE_OTHER)
        .count();
    let lost = results
        .iter()
        .filter(|s| **s == StatusCode::CONFLICT)
        .count();
    assert_eq!((won, lost), (1, 19), "{results:?}");
    let n: i64 = sqlx::query_scalar("select count(*) from bookings")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[sqlx::test]
async fn second_staff_member_takes_the_second_booking(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (ana, service) = common::open_business(&app).await;
    app.signup("Ben", "ben@example.com").await;
    app.post_form(
        "/app/kape-barbers/team",
        &[("email", "ben@example.com")],
        Some(&ana),
    )
    .await;
    let start = three_days_out_at_10();
    for email in ["one@example.com", "two@example.com"] {
        let r = app
            .post_form(
                &format!("/b/kape-barbers/{service}/book"),
                &[("start", &start), ("name", "C"), ("email", email)],
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::SEE_OTHER);
    }
    let staff: i64 = sqlx::query_scalar("select count(distinct staff_id) from bookings")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(staff, 2);
}

#[sqlx::test]
async fn times_outside_opening_hours_are_refused(db: PgPool) {
    let app = TestApp::new(db);
    let (_, service) = common::open_business(&app).await;
    let start = three_days_out_at_10().replace("T", " ");
    let early = start.replacen(&start[11..13], "15", 1); // 15:00 UTC is ~3-4 am in Auckland
    let early = early.replace(" ", "T");
    let r = app
        .post_form(
            &format!("/b/kape-barbers/{service}/book"),
            &[("start", &early), ("name", "C"), ("email", "c@example.com")],
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

#[sqlx::test]
async fn staff_see_and_cancel_bookings(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (ana, service) = common::open_business(&app).await;
    let start = three_days_out_at_10();
    app.post_form(
        &format!("/b/kape-barbers/{service}/book"),
        &[
            ("start", &start),
            ("name", "Ben Cruz"),
            ("email", "ben@example.com"),
        ],
        None,
    )
    .await;
    let list = app.get_as("/app/kape-barbers/bookings", &ana).await;
    assert!(list.body.contains("Ben Cruz") && list.body.contains("confirmed"));

    let id: Uuid = sqlx::query_scalar("select id from bookings")
        .fetch_one(&db)
        .await
        .unwrap();
    let row = app
        .htmx_post(
            &format!("/app/kape-barbers/bookings/{id}/cancel"),
            &[],
            &ana,
        )
        .await;
    assert!(row.body.contains("cancelled"));
    // A cancelled slot can be booked again.
    let again = app
        .post_form(
            &format!("/b/kape-barbers/{service}/book"),
            &[
                ("start", &start),
                ("name", "Di"),
                ("email", "di@example.com"),
            ],
            None,
        )
        .await;
    assert_eq!(again.status, StatusCode::SEE_OTHER);
}

#[sqlx::test]
async fn the_booked_page_hides_the_email(db: PgPool) {
    let app = TestApp::new(db);
    let (_, service) = common::open_business(&app).await;
    let start = three_days_out_at_10();
    let res = app
        .post_form(
            &format!("/b/kape-barbers/{service}/book"),
            &[
                ("start", &start),
                ("name", "Ben Cruz"),
                ("email", "ben@example.com"),
            ],
            None,
        )
        .await;
    let page = app.get(res.location()).await;
    assert!(page.body.contains("b***@example.com"), "{}", page.body);
    assert!(!page.body.contains("ben@example.com"));
}

#[sqlx::test]
async fn staff_cannot_cancel_past_bookings(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (ana, _) = common::open_business(&app).await;
    let id: Uuid = sqlx::query_scalar(
        "insert into bookings (org_id, service_id, staff_id, customer_name, customer_email, starts_at, ends_at)
         select s.org_id, s.id, m.user_id, 'Past', 'past@example.com', now() - interval '2 hours', now() - interval '90 minutes'
           from services s join memberships m on m.org_id = s.org_id
         returning id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let list = app.get_as("/app/kape-barbers/bookings", &ana).await;
    assert!(
        !list.body.contains(&format!("bookings/{id}/cancel")),
        "no Cancel button for the past"
    );
    let r = app
        .htmx_post(
            &format!("/app/kape-barbers/bookings/{id}/cancel"),
            &[],
            &ana,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let status: String = sqlx::query_scalar("select status from bookings where id = $1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "confirmed");
}
