//! Slotbook: online booking for small businesses.

pub mod config;
pub mod db;
pub mod state;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// Build the whole application. `main` serves it; tests call it directly.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Liveness: the process is up.
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// Readiness: we can actually serve traffic, which means reaching the database.
async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match sqlx::query_scalar!("select 1 as \"one!\"").fetch_one(&state.db).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ready" }))),
        Err(e) => {
            tracing::warn!(error = %e, "readiness check failed");
            (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "status": "database unavailable" })))
        }
    }
}
