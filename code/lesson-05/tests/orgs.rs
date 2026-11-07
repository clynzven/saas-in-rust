mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::TestApp;
use sqlx::PgPool;

async fn create_org(app: &TestApp, cookie: &str, name: &str, slug: &str) -> common::TestResponse {
    app.post_form(
        "/orgs",
        &[
            ("name", name),
            ("slug", slug),
            ("timezone", "Pacific/Auckland"),
        ],
        Some(cookie),
    )
    .await
}

#[sqlx::test]
async fn creating_a_business_makes_you_its_owner(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.signup("Ana", "ana@example.com").await;
    let res = create_org(&app, &ana, "Kape Barbers", "kape-barbers").await;
    assert_eq!(res.location(), "/app/kape-barbers");

    let team = app.get_as("/app/kape-barbers/team", &ana).await;
    assert!(team.body.contains("owner"));
    assert!(team.body.contains("Add a team member"));
}

#[sqlx::test]
async fn slugs_are_unique(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.signup("Ana", "ana@example.com").await;
    create_org(&app, &ana, "Kape Barbers", "kape-barbers").await;
    let again = create_org(&app, &ana, "Another", "kape-barbers").await;
    assert_eq!(again.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(again.body.contains("That address is taken"));
}

#[sqlx::test]
async fn other_businesses_are_invisible(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.signup("Ana", "ana@example.com").await;
    let ben = app.signup("Ben", "ben@example.com").await;
    create_org(&app, &ana, "Kape Barbers", "kape-barbers").await;

    // Ben isn't a member: he gets the same 404 as for a business that doesn't exist.
    assert_eq!(
        app.get_as("/app/kape-barbers", &ben).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get_as("/app/nope", &ben).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test]
async fn only_owners_manage_the_team(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.signup("Ana", "ana@example.com").await;
    let ben = app.signup("Ben", "ben@example.com").await;
    app.signup("Cy", "cy@example.com").await;
    create_org(&app, &ana, "Kape Barbers", "kape-barbers").await;

    let added = app
        .post_form(
            "/app/kape-barbers/team",
            &[("email", "BEN@example.com")],
            Some(&ana),
        )
        .await;
    assert_eq!(added.status, StatusCode::SEE_OTHER);
    // Ben can now see the business, but as staff he can't add people.
    assert_eq!(
        app.get_as("/app/kape-barbers", &ben).await.status,
        StatusCode::OK
    );
    let denied = app
        .post_form(
            "/app/kape-barbers/team",
            &[("email", "cy@example.com")],
            Some(&ben),
        )
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn cross_site_posts_are_blocked(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.signup("Ana", "ana@example.com").await;
    let req = Request::post("/logout")
        .header(header::COOKIE, &ana)
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.send(req).await.status, StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn pages_carry_security_headers(db: PgPool) {
    let app = TestApp::new(db);
    let res = app.get("/").await;
    assert!(
        res.headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("default-src 'self'")
    );
    assert_eq!(res.headers[header::X_FRAME_OPTIONS], "DENY");
}

#[sqlx::test]
async fn login_attempts_are_rate_limited(db: PgPool) {
    let app = TestApp::new(db);
    app.signup("Ana", "ana@example.com").await;
    for _ in 0..5 {
        let r = app
            .post_form(
                "/login",
                &[("email", "ana@example.com"), ("password", "guess guess")],
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    // Even the right password is refused once the limit is hit.
    let r = app
        .post_form(
            "/login",
            &[
                ("email", "ana@example.com"),
                ("password", "correct horse battery"),
            ],
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

/// A login form post from a given network address.
fn login_from(ip: &str, password: &str) -> axum::http::Request<axum::body::Body> {
    let body = serde_urlencoded::to_string([("email", "ana@example.com"), ("password", password)])
        .unwrap();
    let addr: std::net::SocketAddr = format!("{ip}:50000").parse().unwrap();
    axum::http::Request::post("/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://localhost:3000")
        .extension(axum::extract::ConnectInfo(addr))
        .body(axum::body::Body::from(body))
        .unwrap()
}

#[sqlx::test]
async fn someone_elses_guesses_dont_lock_you_out(db: PgPool) {
    let app = TestApp::new(db);
    app.signup("Ana", "ana@example.com").await;
    for _ in 0..6 {
        app.send(login_from("203.0.113.9", "guess guess")).await;
    }
    let attacker = app
        .send(login_from("203.0.113.9", "correct horse battery"))
        .await;
    assert_eq!(attacker.status, StatusCode::TOO_MANY_REQUESTS);
    // Ana, on her own network, still gets in.
    let ana = app
        .send(login_from("198.51.100.7", "correct horse battery"))
        .await;
    assert_eq!(ana.status, StatusCode::SEE_OTHER);
}

#[sqlx::test]
async fn signups_are_rate_limited_per_network(db: PgPool) {
    let app = TestApp::new(db);
    for i in 0..30 {
        app.signup("Bot", &format!("bot{i}@example.com")).await;
    }
    let r = app
        .post_form(
            "/signup",
            &[
                ("name", "Bot"),
                ("email", "bot99@example.com"),
                ("password", "correct horse battery"),
            ],
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test]
async fn adding_members_is_rate_limited(db: PgPool) {
    let app = TestApp::new(db);
    let ana = app.signup("Ana", "ana@example.com").await;
    create_org(&app, &ana, "Kape Barbers", "kape-barbers").await;
    for i in 0..30 {
        let email = format!("nobody{i}@example.com");
        app.post_form(
            "/app/kape-barbers/team",
            &[("email", email.as_str())],
            Some(&ana),
        )
        .await;
    }
    let r = app
        .post_form(
            "/app/kape-barbers/team",
            &[("email", "nobody99@example.com")],
            Some(&ana),
        )
        .await;
    assert!(
        r.body.contains("a lot of people in a short time"),
        "{}",
        r.body
    );
}
