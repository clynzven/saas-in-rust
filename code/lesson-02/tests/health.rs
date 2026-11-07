use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use slotbook::AppState;
use slotbook::config::{Config, LogFormat};
use sqlx::PgPool;
use tower::ServiceExt;

fn state(db: PgPool) -> AppState {
    let config = Config {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        database_url: String::new(),
        base_url: "http://localhost:3000".into(),
        log_format: LogFormat::Pretty,
    };
    AppState { db, config: Arc::new(config) }
}

// #[sqlx::test] creates a fresh database for this test, runs our
// migrations, and drops it afterwards.
#[sqlx::test]
async fn ready_when_database_is_up(db: PgPool) {
    let response = slotbook::app(state(db))
        .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
