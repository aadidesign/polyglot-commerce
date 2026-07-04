pub mod handlers;
pub mod models;
pub mod repo;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/auth/register", post(handlers::register))
        .route("/v1/auth/login", post(handlers::login))
        .route("/v1/auth/refresh", post(handlers::refresh))
        .route("/v1/auth/logout", post(handlers::logout))
        .route("/v1/auth/me", get(handlers::me))
}
