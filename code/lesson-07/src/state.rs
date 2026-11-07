use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;

use crate::config::Config;
use crate::security::RateLimiter;

/// Everything a request handler might need. Cloning is cheap:
/// the pool is reference-counted and the rest sits behind Arcs.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    /// Wrong passwords per network and email address.
    pub login_limiter: Arc<RateLimiter>,
    /// Logins, sign-ups and team invites per network (or per user).
    pub ip_limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(db: PgPool, config: Config) -> Self {
        Self {
            db,
            config: Arc::new(config),
            login_limiter: Arc::new(RateLimiter::new(5, Duration::from_secs(15 * 60))),
            ip_limiter: Arc::new(RateLimiter::new(30, Duration::from_secs(15 * 60))),
        }
    }
}
