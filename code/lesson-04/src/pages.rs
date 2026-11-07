//! Public pages that don't need a login.

use askama::Template;
use axum::response::Html;

use crate::error::{AppError, AppResult};
use crate::web::render;

#[derive(Template)]
#[template(path = "home.html")]
struct Home;

pub async fn home() -> AppResult<Html<String>> {
    render(Home)
}

pub async fn not_found() -> AppError {
    AppError::NotFound
}
