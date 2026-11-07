//! Database connection pool and migrations.

use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub async fn connect(url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(10)
        // Fail fast instead of hanging a request when the pool is exhausted.
        .acquire_timeout(Duration::from_secs(5))
        .connect(url)
        .await
        .context("could not connect to the database (is DATABASE_URL right?)")
}

/// Migrations are compiled into the binary, so a deploy is one file.
pub async fn migrate(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
