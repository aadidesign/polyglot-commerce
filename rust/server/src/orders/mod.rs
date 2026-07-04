pub mod handlers;
pub mod models;
pub mod repo;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/orders",
            get(handlers::list_orders).post(handlers::checkout),
        )
        .route("/v1/orders/{id}", get(handlers::get_order))
        .route("/v1/orders/{id}/cancel", post(handlers::cancel_order))
}
