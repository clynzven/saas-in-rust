//! A tiny test client: build the real app on a fresh database and send it requests.
#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use slotbook::AppState;
use slotbook::config::{Config, LogFormat};
use slotbook::mail::Mailer;
use sqlx::PgPool;
use tower::ServiceExt;

pub struct TestApp {
    pub router: Router,
    pub db: PgPool,
    pub state: AppState,
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
        database_max_connections: 5,
        base_url: "http://localhost:3000".into(),
        log_format: LogFormat::Pretty,
        trusted_ip_header: None,
        smtp_url: None,
        mail_from: "Slotbook <bookings@localhost>".into(),
    }
}

impl TestApp {
    pub fn new(db: PgPool) -> Self {
        let state = AppState::new(db.clone(), config(), Mailer::memory());
        Self {
            router: slotbook::app(state.clone()),
            db,
            state,
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

impl TestApp {
    /// Same as post_form, but marked as an HTMX request.
    pub async fn htmx_post(&self, path: &str, form: &[(&str, &str)], cookie: &str) -> TestResponse {
        let body = serde_urlencoded::to_string(form).unwrap();
        let req = Request::post(path)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ORIGIN, "http://localhost:3000")
            .header(header::COOKIE, cookie)
            .header("hx-request", "true")
            .body(Body::from(body))
            .unwrap();
        self.send(req).await
    }

    /// A signed-up owner with a business called "Kape Barbers" (slug kape-barbers).
    pub async fn owner_with_business(&self) -> String {
        let cookie = self.signup("Ana", "ana@example.com").await;
        let res = self
            .post_form(
                "/orgs",
                &[
                    ("name", "Kape Barbers"),
                    ("slug", "kape-barbers"),
                    ("timezone", "Pacific/Auckland"),
                ],
                Some(&cookie),
            )
            .await;
        assert_eq!(res.status, StatusCode::SEE_OTHER);
        cookie
    }
}

impl TestApp {
    /// Run every due background job now, like the worker would.
    pub async fn run_jobs(&self) -> usize {
        slotbook::jobs::run_due(&self.state).await.unwrap()
    }

    pub fn emails(&self) -> Vec<slotbook::mail::Email> {
        self.state.mailer.sent()
    }
}

/// An owner whose business is open every day 09:00-17:00 with a 30-minute haircut.
pub async fn open_business(app: &TestApp) -> (String, uuid::Uuid) {
    let ana = app.owner_with_business().await;
    let mut form = vec![];
    for d in 0..7 {
        form.push((format!("open_{d}"), "on".to_string()));
        form.push((format!("opens_{d}"), "09:00".to_string()));
        form.push((format!("closes_{d}"), "17:00".to_string()));
    }
    let form: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    app.post_form("/app/kape-barbers/hours", &form, Some(&ana))
        .await;
    app.post_form(
        "/app/kape-barbers/services",
        &[
            ("name", "Haircut"),
            ("duration_minutes", "30"),
            ("price", "35"),
        ],
        Some(&ana),
    )
    .await;
    let id = sqlx::query_scalar("select id from services")
        .fetch_one(&app.db)
        .await
        .unwrap();
    (ana, id)
}

/// 10:00 in Auckland, three days from now, as RFC 3339.
pub fn three_days_out_at_10() -> String {
    let day = chrono::Utc::now()
        .with_timezone(&chrono_tz::Pacific::Auckland)
        .date_naive()
        + chrono::Days::new(3);
    let local = day.and_hms_opt(10, 0, 0).unwrap();
    slotbook::availability::to_utc(chrono_tz::Pacific::Auckland, local)
        .unwrap()
        .to_rfc3339()
}

/// The calendar day in Auckland for an RFC 3339 instant.
pub fn local_day(rfc3339: &str) -> String {
    let t: chrono::DateTime<chrono::Utc> = rfc3339.parse().unwrap();
    t.with_timezone(&chrono_tz::Pacific::Auckland)
        .date_naive()
        .to_string()
}
