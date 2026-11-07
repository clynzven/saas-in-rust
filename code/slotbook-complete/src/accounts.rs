//! Sign up, log in, log out.

use askama::Template;
use axum::Form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::AppState;
use crate::auth::{self, CurrentUser};
use crate::error::AppResult;
use crate::forms;
use crate::web::render;

#[derive(Template, Default)]
#[template(path = "signup.html")]
struct SignupPage {
    error: Option<String>,
    name: String,
    email: String,
}

#[derive(Deserialize)]
pub struct SignupForm {
    name: String,
    email: String,
    password: String,
}

pub async fn signup_page(user: Option<CurrentUser>) -> AppResult<Response> {
    if user.is_some() {
        return Ok(Redirect::to("/dashboard").into_response());
    }
    Ok(render(SignupPage::default())?.into_response())
}

pub async fn signup(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(f): Form<SignupForm>,
) -> AppResult<Response> {
    let again = |error: String| -> AppResult<Response> {
        let page = SignupPage {
            error: Some(error),
            name: f.name.clone(),
            email: f.email.clone(),
        };
        Ok((StatusCode::UNPROCESSABLE_ENTITY, render(page)?).into_response())
    };
    let name = match forms::text(&f.name, "your name", 80) {
        Ok(n) => n,
        Err(e) => return again(e),
    };
    let email = match forms::email(&f.email) {
        Ok(e) => e,
        Err(e) => return again(e),
    };
    if let Err(e) = forms::password(&f.password) {
        return again(e);
    }

    let hash = auth::hash(f.password.clone()).await?;
    let inserted = sqlx::query_scalar!(
        "insert into users (email, name, password_hash) values ($1, $2, $3) returning id",
        email,
        name,
        hash,
    )
    .fetch_one(&state.db)
    .await;

    let user_id = match inserted {
        Ok(id) => id,
        Err(e) if is_unique_violation(&e, "users_email_key") => {
            return again("An account with this email already exists. Try logging in.".into());
        }
        Err(e) => return Err(e.into()),
    };
    tracing::info!(%user_id, "new account");
    let jar = auth::start_session(&state, user_id, jar).await?;
    Ok((jar, Redirect::to("/dashboard")).into_response())
}

#[derive(Template, Default)]
#[template(path = "login.html")]
struct LoginPage {
    error: Option<String>,
    email: String,
}

#[derive(Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
}

pub async fn login_page(user: Option<CurrentUser>) -> AppResult<Response> {
    if user.is_some() {
        return Ok(Redirect::to("/dashboard").into_response());
    }
    Ok(render(LoginPage::default())?.into_response())
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(f): Form<LoginForm>,
) -> AppResult<Response> {
    let email = f.email.trim().to_lowercase();
    let row = sqlx::query!(
        "select id, password_hash from users where lower(email) = $1",
        email
    )
    .fetch_optional(&state.db)
    .await?;
    let (id, hash) = match row {
        Some(r) => (Some(r.id), Some(r.password_hash)),
        None => (None, None),
    };
    match (auth::verify(f.password, hash).await?, id) {
        (true, Some(user_id)) => {
            let jar = auth::start_session(&state, user_id, jar).await?;
            Ok((jar, Redirect::to("/dashboard")).into_response())
        }
        // Same message either way: we don't say whether the email exists.
        _ => {
            let page = LoginPage {
                error: Some("Email or password is incorrect.".into()),
                email: f.email,
            };
            Ok((StatusCode::UNAUTHORIZED, render(page)?).into_response())
        }
    }
}

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<Response> {
    let jar = auth::end_session(&state.db, jar).await?;
    Ok((jar, Redirect::to("/")).into_response())
}

pub fn is_unique_violation(e: &sqlx::Error, constraint: &str) -> bool {
    e.as_database_error().and_then(|d| d.constraint()) == Some(constraint)
}
