//! Slotbook: online booking for small businesses.

mod accounts;
pub mod auth;
pub mod availability;
mod booking;
mod cancel;
mod catalog;
pub mod config;
mod dashboard;
pub mod db;
pub mod error;
pub mod forms;
pub mod jobs;
pub mod mail;
mod orgs;
mod pages;
pub mod security;
pub mod state;
pub mod web;

use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;

pub use state::AppState;

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Build the whole application. `main` serves it; tests call it directly.
pub fn app(state: AppState) -> Router {
    // Pages for business owners and staff (log in required).
    let team = Router::new()
        .route("/dashboard", get(dashboard::dashboard))
        .route("/orgs/new", get(orgs::new_page))
        .route("/orgs", post(orgs::create))
        .route("/app/{slug}", get(orgs::home))
        .route(
            "/app/{slug}/services",
            get(catalog::services).post(catalog::add_service),
        )
        .route(
            "/app/{slug}/services/{id}/toggle",
            post(catalog::toggle_service),
        )
        .route(
            "/app/{slug}/hours",
            get(catalog::hours).post(catalog::save_hours),
        )
        .route("/app/{slug}/bookings", get(booking::bookings))
        .route(
            "/app/{slug}/bookings/{id}/cancel",
            post(booking::cancel_by_staff),
        )
        .route("/app/{slug}/team", get(orgs::team).post(orgs::add_member))
        .route(
            "/app/{slug}/team/{user_id}/remove",
            post(orgs::remove_member),
        );

    // Pages for customers and visitors.
    let public = Router::new()
        .route("/", get(pages::home))
        .route("/signup", get(accounts::signup_page).post(accounts::signup))
        .route("/login", get(accounts::login_page).post(accounts::login))
        .route("/logout", post(accounts::logout))
        .route("/b/{slug}", get(booking::org_page))
        .route("/b/{slug}/{service_id}", get(booking::service_page))
        .route(
            "/b/{slug}/{service_id}/book",
            get(booking::book_page).post(booking::book),
        )
        .route("/b/{slug}/booked/{id}", get(booking::booked))
        .route("/cancel/{token}", get(cancel::show).post(cancel::confirm));

    // Static files rarely change: let browsers cache them for a day.
    let assets = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=86400"),
        ))
        .service(ServeDir::new(static_dir()));

    Router::new()
        .merge(team)
        .merge(public)
        .route("/health", get(health))
        .route("/ready", get(ready))
        .nest_service("/static", assets)
        .fallback(pages::not_found)
        .layer(middleware::from_fn_with_state(state.clone(), security::same_origin_only))
        .layer(middleware::from_fn(security::headers))
        // Layers run top to bottom for requests, bottom to top for responses.
        .layer(
            ServiceBuilder::new()
                // Give every request an id, log it, and send it back to the client.
                .layer(SetRequestIdLayer::new(REQUEST_ID, MakeRequestUuid))
                .layer(TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                    let id = req.headers().get(&REQUEST_ID).and_then(|v| v.to_str().ok()).unwrap_or("-");
                    tracing::info_span!("request", method = %req.method(), path = security::loggable_path(req.uri().path()), id)
                }).on_response(DefaultOnResponse::new().level(Level::INFO)))
                .layer(PropagateRequestIdLayer::new(REQUEST_ID))
                // A panic in one handler becomes a 500, not a dead connection.
                .layer(CatchPanicLayer::new())
                // Our forms are small; refuse anything over 64 KB.
                .layer(RequestBodyLimitLayer::new(64 * 1024))
                .layer(CompressionLayer::new())
                // Slow clients and slow queries can't hold a worker forever.
                .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(15))),
        )
        .with_state(state)
}

/// Liveness: the process is up.
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// Readiness: we can actually serve traffic, which means reaching the database.
async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match sqlx::query_scalar!("select 1 as \"one!\"")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ready" }))),
        Err(e) => {
            tracing::warn!(error = %e, "readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "database unavailable" })),
            )
        }
    }
}

/// Static files live next to Cargo.toml in development and next to the
/// binary in the container image; STATIC_DIR overrides both.
fn static_dir() -> String {
    std::env::var("STATIC_DIR")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/static").to_string())
}
