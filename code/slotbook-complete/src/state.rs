use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;

/// Everything a request handler might need. Cloning is cheap:
/// the pool is reference-counted and the config sits behind an Arc.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
}
