pub mod handlers;
pub mod models;
pub mod repo;

use axum::routing::{delete, get};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/cart",
            get(handlers::get_cart).delete(handlers::clear_cart),
        )
        .route("/v1/cart/items", axum::routing::post(handlers::add_item))
        .route(
            "/v1/cart/items/{product_id}",
            delete(handlers::remove_item).patch(handlers::update_item),
        )
}
