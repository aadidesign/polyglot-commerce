pub mod cache;
pub mod handlers;
pub mod models;
pub mod repo;

use axum::routing::get;
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/products",
            get(handlers::list_products).post(handlers::create_product),
        )
        .route(
            "/v1/products/{id}",
            get(handlers::get_product)
                .patch(handlers::update_product)
                .delete(handlers::archive_product),
        )
        .route(
            "/v1/categories",
            get(handlers::list_categories).post(handlers::create_category),
        )
}
