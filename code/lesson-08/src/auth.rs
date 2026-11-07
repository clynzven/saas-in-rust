//! Passwords, sessions and the `CurrentUser` extractor.

use std::sync::LazyLock;

use anyhow::anyhow;
use argon2::Argon2;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, AppResult};

pub const SESSION_COOKIE: &str = "sb_session";
const SESSION_DAYS: i32 = 30;

// ---------------------------------------------------------------- passwords

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("hashing failed: {e}"))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}

/// Argon2 is slow on purpose (that's what makes guessing expensive), so we
/// run it on the blocking thread pool instead of stalling the async workers.
pub async fn hash(password: String) -> AppResult<String> {
    let hashed = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(hashed)
}

pub async fn verify(password: String, hash: Option<String>) -> AppResult<bool> {
    // No such user? Check against a dummy hash anyway, so the response takes
    // the same time and doesn't reveal which emails have accounts.
    let (hash, real) = match hash {
        Some(h) => (h, true),
        None => (DUMMY_HASH.clone(), false),
    };
    let ok = tokio::task::spawn_blocking(move || verify_password(&password, &hash))
        .await
        .map_err(anyhow::Error::from)?;
    Ok(ok && real)
}

static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| hash_password("no-one-knows-this-password").expect("argon2 works"));

// ---------------------------------------------------------------- sessions

/// A random token for the cookie, and the hash we store.
pub fn new_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = token_hash(&token);
    (token, hash)
}

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub async fn start_session(
    state: &AppState,
    user_id: Uuid,
    jar: CookieJar,
) -> AppResult<CookieJar> {
    let (token, hash) = new_token();
    sqlx::query!(
        "insert into sessions (token_hash, user_id, expires_at)
         values ($1, $2, now() + make_interval(days => $3))",
        hash,
        user_id,
        SESSION_DAYS,
    )
    .execute(&state.db)
    .await?;

    let cookie = Cookie::build((SESSION_COOKIE, token))
        .path("/")
        .http_only(true) // JavaScript can't read it
        .secure(state.config.base_url.starts_with("https://")) // HTTPS only in production
        .same_site(SameSite::Lax) // not sent on cross-site POSTs
        .max_age(time::Duration::days(SESSION_DAYS.into()))
        .build();
    Ok(jar.add(cookie))
}

pub async fn end_session(db: &PgPool, jar: CookieJar) -> AppResult<CookieJar> {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        sqlx::query!(
            "delete from sessions where token_hash = $1",
            token_hash(c.value())
        )
        .execute(db)
        .await?;
    }
    Ok(jar.remove(Cookie::build(SESSION_COOKIE).path("/")))
}

// ---------------------------------------------------------------- extractor

/// Add `user: CurrentUser` to a handler and it only runs for logged-in users.
#[derive(Clone, Debug)]
pub struct CurrentUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar.get(SESSION_COOKIE).ok_or(AppError::Unauthorized)?;
        let user = sqlx::query_as!(
            CurrentUser,
            "select u.id, u.name, u.email
               from sessions s join users u on u.id = s.user_id
              where s.token_hash = $1 and s.expires_at > now()",
            token_hash(token.value()),
        )
        .fetch_optional(&state.db)
        .await?;
        user.ok_or(AppError::Unauthorized)
    }
}

/// `Option<CurrentUser>` for pages that work either way.
impl OptionalFromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Option<Self>, AppError> {
        match <CurrentUser as FromRequestParts<AppState>>::from_request_parts(parts, state).await {
            Ok(user) => Ok(Some(user)),
            Err(AppError::Unauthorized) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
