//! Helpers for HTML responses.

use askama::Template;
use axum::response::Html;

use crate::error::AppResult;

/// Render any template into an HTML response, turning template errors into AppError.
pub fn render(t: impl Template) -> AppResult<Html<String>> {
    Ok(Html(t.render()?))
}

/// HTMX marks its requests with `HX-Request: true`.
pub fn is_htmx(headers: &axum::http::HeaderMap) -> bool {
    headers.get("hx-request").is_some_and(|v| v == "true")
}
