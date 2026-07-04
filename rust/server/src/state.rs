use std::sync::Arc;

use common::auth::{HasJwt, JwtManager};
use common::telemetry::PrometheusHandle;
use redis::aio::ConnectionManager;
use sqlx::PgPool;

pub struct Inner {
    pub pool: PgPool,
    pub jwt: JwtManager,
    /// `None` when Redis is not configured - the service degrades to DB-only.
    pub redis: Option<ConnectionManager>,
    pub cache_ttl_secs: u64,
    pub metrics: PrometheusHandle,
}

pub type AppState = Arc<Inner>;

impl HasJwt for Inner {
    fn jwt(&self) -> &JwtManager {
        &self.jwt
    }
}
