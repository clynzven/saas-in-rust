//! A tiny test client: build the real app on a fresh database and send it requests.
#![allow(dead_code)]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use slotbook::AppState;
use slotbook::config::{Config, LogFormat};
use sqlx::PgPool;
use tower::ServiceExt;

pub struct TestApp {
    pub router: Router,
    pub db: PgPool,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: String,
}

impl TestResponse {
    pub fn location(&self) -> &str {
        self.headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }
}

pub fn config() -> Config {
    Config {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        database_url: String::new(),
        base_url: "http://localhost:3000".into(),
        log_format: LogFormat::Pretty,
    }
}

impl TestApp {
    pub fn new(db: PgPool) -> Self {
        let state = AppState {
            db: db.clone(),
            config: Arc::new(config()),
        };
        Self {
            router: slotbook::app(state),
            db,
        }
    }

    pub async fn send(&self, req: Request<Body>) -> TestResponse {
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        TestResponse {
            status,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    pub async fn get(&self, path: &str) -> TestResponse {
        self.send(Request::get(path).body(Body::empty()).unwrap())
            .await
    }
}
