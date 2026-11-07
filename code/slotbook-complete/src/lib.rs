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

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// Build the whole application. `main` serves it; tests call it directly.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(pages::home))
        .route("/signup", get(accounts::signup_page).post(accounts::signup))
        .route("/login", get(accounts::login_page).post(accounts::login))
        .route("/logout", post(accounts::logout))
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
        .route("/b/{slug}", get(booking::org_page))
        .route("/b/{slug}/{service_id}", get(booking::service_page))
        .route(
            "/b/{slug}/{service_id}/book",
            get(booking::book_page).post(booking::book),
        )
        .route("/b/{slug}/booked/{id}", get(booking::booked))
        .route("/cancel/{token}", get(cancel::show).post(cancel::confirm))
        .route("/app/{slug}/team", get(orgs::team).post(orgs::add_member))
        .route(
            "/app/{slug}/team/{user_id}/remove",
            post(orgs::remove_member),
        )
        .route("/health", get(health))
        .route("/ready", get(ready))
        .nest_service("/static", ServeDir::new(static_dir()))
        .fallback(pages::not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security::same_origin_only,
        ))
        .layer(middleware::from_fn(security::headers))
        .layer(TraceLayer::new_for_http())
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
