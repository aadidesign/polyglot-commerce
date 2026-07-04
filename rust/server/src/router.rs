use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;
use crate::{auth, cart, orders, products};

pub fn build(state: AppState) -> Router {
    let api = Router::new()
        .merge(auth::routes())
        .merge(products::routes())
        .merge(cart::routes())
        .merge(orders::routes())
        .route("/healthz", get(liveness))
        .route("/readyz", get(readiness))
        .route("/metrics", get(metrics))
        .with_state(state);

    common::http::with_middleware(api)
}

async fn liveness() -> &'static str {
    "ok"
}

async fn readiness(State(st): State<AppState>) -> impl IntoResponse {
    if common::db::ping(&st.pool).await {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "degraded")
    }
}

async fn metrics(State(st): State<AppState>) -> String {
    st.metrics.render()
}
