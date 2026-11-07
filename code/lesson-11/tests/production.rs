//! Behaviour we rely on in production, beyond the features themselves.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::TestApp;
use sqlx::PgPool;

#[sqlx::test]
async fn every_response_carries_a_request_id(db: PgPool) {
    let app = TestApp::new(db);
    let res = app.get("/health").await;
    let id = res.headers["x-request-id"].to_str().unwrap();
    assert_eq!(id.len(), 36, "a UUID: {id}");

    // An id from a load balancer is kept, so logs line up across systems.
    let req = Request::get("/health")
        .header("x-request-id", "from-the-proxy")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.send(req).await.headers["x-request-id"],
        "from-the-proxy"
    );
}

#[sqlx::test]
async fn huge_bodies_are_refused(db: PgPool) {
    let app = TestApp::new(db);
    let big = "x".repeat(100 * 1024);
    let res = app
        .post_form("/login", &[("email", &big), ("password", "x")], None)
        .await;
    assert_eq!(res.status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[sqlx::test]
async fn pages_are_compressed_when_the_browser_asks(db: PgPool) {
    let app = TestApp::new(db);
    let req = Request::get("/")
        .header(header::ACCEPT_ENCODING, "gzip")
        .body(Body::empty())
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.headers[header::CONTENT_ENCODING], "gzip");
}

#[sqlx::test]
async fn static_files_are_cacheable(db: PgPool) {
    let app = TestApp::new(db);
    let res = app.get("/static/app.css").await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.headers[header::CACHE_CONTROL], "public, max-age=86400");
}

#[sqlx::test]
async fn housekeeping_removes_expired_sessions(db: PgPool) {
    let app = TestApp::new(db.clone());
    let cookie = app.signup("Ana", "ana@example.com").await;
    sqlx::query("update sessions set expires_at = now() - interval '1 minute'")
        .execute(&db)
        .await
        .unwrap();

    let (sessions, _) = slotbook::jobs::housekeeping(&db).await.unwrap();
    assert_eq!(sessions, 1);
    assert_eq!(app.get_as("/dashboard", &cookie).await.location(), "/login");
}
