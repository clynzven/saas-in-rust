mod common;

use axum::http::StatusCode;
use common::TestApp;
use sqlx::PgPool;

#[sqlx::test]
async fn htmx_add_service_returns_just_the_section(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.owner_with_business().await;
    let res = app
        .htmx_post(
            "/app/kape-barbers/services",
            &[
                ("name", "Skin fade"),
                ("duration_minutes", "45"),
                ("price", "40"),
            ],
            &ana,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.body.contains("Skin fade") && res.body.contains("$40.00"));
    // A fragment, not a whole page: no <html>, no navigation.
    assert!(!res.body.contains("<html"));
    assert!(res.body.trim_start().starts_with("<div id=\"services\">"));
}

#[sqlx::test]
async fn invalid_service_shows_the_error_inline(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.owner_with_business().await;
    let res = app
        .htmx_post(
            "/app/kape-barbers/services",
            &[
                ("name", "Cut"),
                ("duration_minutes", "30"),
                ("price", "a lot"),
            ],
            &ana,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(res.body.contains("Enter a price like 35 or 35.50"));
}

#[sqlx::test]
async fn hours_are_saved_for_ticked_days_only(db: PgPool) {
    let app = TestApp::new(db.clone());
    let ana = app.owner_with_business().await;
    let res = app
        .htmx_post(
            "/app/kape-barbers/hours",
            &[
                ("open_0", "on"),
                ("opens_0", "09:00"),
                ("closes_0", "17:00"),
                ("opens_6", "10:00"),
                ("closes_6", "14:00"),
            ],
            &ana,
        )
        .await;
    assert!(res.body.contains("Opening hours saved"));
    let n: i64 = sqlx::query_scalar("select count(*) from opening_hours")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[sqlx::test]
async fn staff_can_look_but_not_change(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.owner_with_business().await;
    let ben = app.signup("Ben", "ben@example.com").await;
    app.post_form(
        "/app/kape-barbers/team",
        &[("email", "ben@example.com")],
        Some(&ana),
    )
    .await;

    assert_eq!(
        app.get_as("/app/kape-barbers/services", &ben).await.status,
        StatusCode::OK
    );
    let res = app
        .htmx_post(
            "/app/kape-barbers/services",
            &[("name", "Cut"), ("duration_minutes", "30"), ("price", "30")],
            &ben,
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn toggling_a_service_returns_its_row(db: PgPool) {
    let app = TestApp::new(db.clone());
    let ana = app.owner_with_business().await;
    app.htmx_post(
        "/app/kape-barbers/services",
        &[
            ("name", "Beard trim"),
            ("duration_minutes", "15"),
            ("price", "20"),
        ],
        &ana,
    )
    .await;
    let id: uuid::Uuid = sqlx::query_scalar("select id from services")
        .fetch_one(&db)
        .await
        .unwrap();
    let res = app
        .htmx_post(
            &format!("/app/kape-barbers/services/{id}/toggle"),
            &[],
            &ana,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(
        res.body.trim_start().starts_with("<tr id=\"service-"),
        "got: {}",
        res.body
    );
    assert!(res.body.contains("Off · turn on"));
}
