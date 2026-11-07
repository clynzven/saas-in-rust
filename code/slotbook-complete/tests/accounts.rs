mod common;

use axum::http::StatusCode;
use common::{TestApp, session_cookie};
use sqlx::PgPool;

#[sqlx::test]
async fn signup_logs_you_in(db: PgPool) {
    let app = TestApp::new(db);
    let cookie = app.signup("Ana", "ana@example.com").await;
    let page = app.get_as("/dashboard", &cookie).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("Hi, Ana"));
}

#[sqlx::test]
async fn passwords_are_hashed_not_stored(db: PgPool) {
    let app = TestApp::new(db.clone());
    app.signup("Ana", "ana@example.com").await;
    let hash: String = sqlx::query_scalar("select password_hash from users")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains("correct horse"));
}

#[sqlx::test]
async fn emails_are_unique_ignoring_case(db: PgPool) {
    let app = TestApp::new(db);
    app.signup("Ana", "ana@example.com").await;
    let res = app
        .post_form(
            "/signup",
            &[
                ("name", "Ana 2"),
                ("email", "ANA@example.com"),
                ("password", "another long one"),
            ],
            None,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(res.body.contains("already exists"));
}

#[sqlx::test]
async fn login_and_logout(db: PgPool) {
    let app = TestApp::new(db);
    app.signup("Ana", "ana@example.com").await;

    let bad = app
        .post_form(
            "/login",
            &[("email", "ana@example.com"), ("password", "wrong password")],
            None,
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert!(bad.body.contains("Email or password is incorrect"));

    let ok = app
        .post_form(
            "/login",
            &[
                ("email", "Ana@Example.com"),
                ("password", "correct horse battery"),
            ],
            None,
        )
        .await;
    assert_eq!(ok.location(), "/dashboard");
    let cookie = session_cookie(&ok);
    assert_eq!(
        app.get_as("/dashboard", &cookie).await.status,
        StatusCode::OK
    );

    app.post_form("/logout", &[], Some(&cookie)).await;
    // The session row is gone, so the old cookie no longer works.
    let after = app.get_as("/dashboard", &cookie).await;
    assert_eq!(after.location(), "/login");
}

#[sqlx::test]
async fn dashboard_needs_a_login(db: PgPool) {
    let app = TestApp::new(db);
    let res = app.get("/dashboard").await;
    assert_eq!(res.status, StatusCode::SEE_OTHER);
    assert_eq!(res.location(), "/login");
}
