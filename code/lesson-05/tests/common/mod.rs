//! A tiny test client: build the real app on a fresh database and send it requests.
#![allow(dead_code)]

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
        trusted_ip_header: None,
    }
}

impl TestApp {
    pub fn new(db: PgPool) -> Self {
        let state = AppState::new(db.clone(), config());
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

impl TestApp {
    pub async fn post_form(
        &self,
        path: &str,
        form: &[(&str, &str)],
        cookie: Option<&str>,
    ) -> TestResponse {
        let body = serde_urlencoded::to_string(form).unwrap();
        let mut req = Request::post(path)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ORIGIN, "http://localhost:3000");
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        self.send(req.body(Body::from(body)).unwrap()).await
    }

    pub async fn get_as(&self, path: &str, cookie: &str) -> TestResponse {
        self.send(
            Request::get(path)
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    /// Sign up a new user and return their session cookie ("sb_session=...").
    pub async fn signup(&self, name: &str, email: &str) -> String {
        let res = self
            .post_form(
                "/signup",
                &[
                    ("name", name),
                    ("email", email),
                    ("password", "correct horse battery"),
                ],
                None,
            )
            .await;
        assert_eq!(
            res.status,
            StatusCode::SEE_OTHER,
            "signup failed: {}",
            res.body
        );
        session_cookie(&res)
    }
}

pub fn session_cookie(res: &TestResponse) -> String {
    let set = res
        .headers
        .get(header::SET_COOKIE)
        .expect("a Set-Cookie header")
        .to_str()
        .unwrap();
    set.split(';').next().unwrap().to_string()
}
