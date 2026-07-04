//! PostgreSQL connection pool construction with sane production defaults.

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
}

impl DbConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            url: crate::env::required("DATABASE_URL")?,
            max_connections: crate::env::parse_or("DB_MAX_CONNECTIONS", 20),
            min_connections: crate::env::parse_or("DB_MIN_CONNECTIONS", 2),
            connect_timeout: Duration::from_secs(crate::env::parse_or(
                "DB_CONNECT_TIMEOUT_SECS",
                5,
            )),
        })
    }
}

/// Build a connection pool. The pool is lazy: it does not block startup waiting
/// for the database, so readiness probes (not liveness) gate traffic.
pub async fn connect(cfg: &DbConfig) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(cfg.connect_timeout)
        .test_before_acquire(true)
        .connect(&cfg.url)
        .await?;
    Ok(pool)
}

/// Liveness/readiness check: a cheap round-trip to the database.
pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false)
}
