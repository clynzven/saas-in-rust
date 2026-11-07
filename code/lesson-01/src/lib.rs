//! Slotbook: online booking for small businesses.

pub mod config;

use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use tower_http::trace::TraceLayer;

/// Build the whole application. `main` serves it; tests call it directly.
pub fn app() -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}
