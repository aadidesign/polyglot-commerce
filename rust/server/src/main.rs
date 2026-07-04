//! E-commerce backend (Rust / axum).
//!
//! A modular monolith: clear bounded modules (auth, products, cart, orders)
//! over one PostgreSQL database, with JWT auth + RBAC, Redis caching, keyset
//! pagination, RFC 7807 errors, Prometheus metrics, structured logging, and
//! graceful shutdown. Checkout is a single ACID transaction (no oversell).

mod auth;
mod cart;
mod config;
mod orders;
mod products;
mod router;
mod state;

use std::sync::Arc;

use common::auth::JwtManager;
use common::{db, env, telemetry};

use crate::config::Config;
use crate::state::Inner;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `ecommerce healthcheck` - used by the Docker HEALTHCHECK.
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        let port: u16 = env::parse_or("PORT", 8080);
        tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
        return Ok(());
    }

    env::load_dotenv();
    let metrics = telemetry::init("ecommerce-rust");
    let cfg = Config::from_env()?;

    let pool = db::connect(&cfg.db).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("migrations applied");

    let redis = match &cfg.redis_url {
        Some(url) => match redis::Client::open(url.clone()) {
            Ok(client) => match redis::aio::ConnectionManager::new(client).await {
                Ok(cm) => {
                    tracing::info!("connected to Redis");
                    Some(cm)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Redis unavailable; caching disabled");
                    None
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "invalid REDIS_URL; caching disabled");
                None
            }
        },
        None => None,
    };

    let port = cfg.port;
    let state = Arc::new(Inner {
        pool,
        jwt: JwtManager::new(cfg.jwt),
        redis,
        cache_ttl_secs: cfg.cache_ttl_secs,
        metrics,
    });

    common::http::serve(port, router::build(state)).await
}
