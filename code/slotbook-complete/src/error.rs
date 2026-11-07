//! One error type for every handler. Each variant knows its HTTP status,
//! and internal details are logged, never shown to visitors.

use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("please log in")]
    Unauthorized,
    #[error("you don't have access to this")]
    Forbidden,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            other => AppError::Internal(other.into()),
        }
    }
}

impl From<askama::Error> for AppError {
    fn from(e: askama::Error) -> Self {
        AppError::Internal(e.into())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage<'a> {
    title: &'a str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, title) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Page not found"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Please log in"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "No access"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "Something's not right"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "That didn't work"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong"),
        };
        let message = match &self {
            AppError::Internal(e) => {
                // The full chain goes to the logs; the visitor gets a calm message.
                tracing::error!(error = ?e, "internal error");
                "We've logged the problem. Please try again in a moment.".to_string()
            }
            other => other.to_string(),
        };
        let body = ErrorPage { title, message }
            .render()
            .unwrap_or_else(|_| title.to_string());
        (status, Html(body)).into_response()
    }
}
