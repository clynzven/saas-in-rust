mod common;

use axum::http::StatusCode;
use sqlx::PgPool;

#[sqlx::test]
async fn home_page_renders(db: PgPool) {
    let app = common::TestApp::new(db);
    let res = app.get("/").await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.body.contains("Bookings without the back-and-forth"));
}

#[sqlx::test]
async fn unknown_pages_get_a_friendly_404(db: PgPool) {
    let app = common::TestApp::new(db);
    let res = app.get("/no-such-page").await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert!(res.body.contains("Page not found"));
}
