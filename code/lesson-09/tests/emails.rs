mod common;

use axum::http::StatusCode;
use common::{TestApp, open_business, three_days_out_at_10};
use sqlx::PgPool;

async fn book(app: &TestApp, service: uuid::Uuid) -> common::TestResponse {
    let start = three_days_out_at_10();
    app.post_form(
        &format!("/b/kape-barbers/{service}/book"),
        &[
            ("start", &start),
            ("name", "Ben Cruz"),
            ("email", "ben@example.com"),
        ],
        None,
    )
    .await
}

/// Pull the /cancel/... link out of an email body.
fn cancel_link(text: &str) -> String {
    let start = text.find("/cancel/").unwrap();
    text[start..].split_whitespace().next().unwrap().to_string()
}

#[sqlx::test]
async fn booking_queues_a_confirmation_email(db: PgPool) {
    let app = TestApp::new(db);
    let (_, service) = open_business(&app).await;
    book(&app, service).await;

    // Nothing is sent during the request: the email waits in the queue.
    assert!(app.emails().is_empty());
    assert_eq!(app.run_jobs().await, 1);

    let sent = app.emails();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "ben@example.com");
    assert!(
        sent[0]
            .subject
            .starts_with("Booked: Haircut at Kape Barbers")
    );
    assert!(sent[0].text.contains("/cancel/"));
    // Done jobs don't run twice.
    assert_eq!(app.run_jobs().await, 0);
}

#[sqlx::test]
async fn the_secret_token_is_removed_once_sent(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (_, service) = open_business(&app).await;
    book(&app, service).await;
    app.run_jobs().await;
    let payload: serde_json::Value = sqlx::query_scalar("select payload from jobs")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(payload.get("cancel_token").is_none(), "{payload}");
}

#[sqlx::test]
async fn customers_cancel_with_the_emailed_link(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (_, service) = open_business(&app).await;
    book(&app, service).await;
    app.run_jobs().await;
    let link = cancel_link(&app.emails()[0].text);

    let page = app.get(&link).await;
    assert!(page.body.contains("Cancel this booking"));
    let done = app.post_form(&link, &[], None).await;
    assert!(done.body.contains("Your booking is cancelled"));
    // A second click doesn't cancel twice or send another email.
    let again = app.post_form(&link, &[], None).await;
    assert!(again.body.contains("already cancelled"));

    app.run_jobs().await;
    let sent = app.emails();
    assert_eq!(sent.len(), 2);
    assert!(sent[1].subject.starts_with("Cancelled:"));
}

#[sqlx::test]
async fn unknown_cancel_links_are_404(db: PgPool) {
    let app = TestApp::new(db);
    assert_eq!(
        app.get("/cancel/not-a-real-token").await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test]
async fn failing_jobs_back_off_and_eventually_stop(db: PgPool) {
    let app = TestApp::new(db.clone());
    // A job for a booking that doesn't exist will fail every time.
    let job = slotbook::jobs::Job::BookingCancelled {
        booking_id: uuid::Uuid::new_v4(),
    };
    let mut conn = db.acquire().await.unwrap();
    slotbook::jobs::enqueue(&mut conn, &job).await.unwrap();

    for attempt in 1..=slotbook::jobs::MAX_ATTEMPTS {
        assert_eq!(app.run_jobs().await, 1, "attempt {attempt}");
        let (wait, error): (f64, String) = sqlx::query_as(
            "select extract(epoch from run_at - now())::float8, last_error from jobs",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(error.contains("booking not found"));
        // It's scheduled for later, so it isn't due yet...
        assert!(wait > 20.0);
        // ...but we fast-forward to keep the test quick.
        sqlx::query("update jobs set run_at = now()")
            .execute(&db)
            .await
            .unwrap();
    }
    // After MAX_ATTEMPTS the job is left alone, with its last error kept for a human.
    assert_eq!(app.run_jobs().await, 0);
}

#[sqlx::test]
async fn given_up_jobs_forget_the_secret(db: PgPool) {
    let app = TestApp::new(db.clone());
    // A confirmation for a booking that doesn't exist fails every time.
    let job = slotbook::jobs::Job::BookingConfirmation {
        booking_id: uuid::Uuid::new_v4(),
        cancel_token: "secret-token".into(),
    };
    let mut conn = db.acquire().await.unwrap();
    slotbook::jobs::enqueue(&mut conn, &job).await.unwrap();
    for _ in 1..=slotbook::jobs::MAX_ATTEMPTS {
        app.run_jobs().await;
        sqlx::query("update jobs set run_at = now()")
            .execute(&db)
            .await
            .unwrap();
    }
    let has_token: bool = sqlx::query_scalar("select payload ? 'cancel_token' from jobs")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!has_token, "the raw token must not stay in the database");
}
