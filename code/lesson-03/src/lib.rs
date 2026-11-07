//! Slotbook: online booking for small businesses.

pub mod config;
pub mod db;
pub mod error;
mod pages;
pub mod state;
pub mod web;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// Build the whole application. `main` serves it; tests call it directly.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(pages::home))
        .route("/health", get(health))
        .route("/ready", get(ready))
        .nest_service("/static", ServeDir::new(static_dir()))
        .fallback(pages::not_found)
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
