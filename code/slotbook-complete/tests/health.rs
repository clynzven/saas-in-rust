mod common;

use axum::http::StatusCode;
use sqlx::PgPool;

// #[sqlx::test] creates a fresh database for this test, runs our
// migrations, and drops it afterwards.
#[sqlx::test]
async fn health_and_ready(db: PgPool) {
    let app = common::TestApp::new(db);
    assert_eq!(app.get("/health").await.status, StatusCode::OK);
    let ready = app.get("/ready").await;
    assert_eq!(ready.status, StatusCode::OK);
    assert!(ready.body.contains("ready"));
}
