//! Order handlers. All require authentication; orders are scoped to the
//! authenticated user.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use common::auth::AuthUser;
use common::pagination::{Cursor, Page, PageQuery, DEFAULT_LIMIT, MAX_LIMIT};
use common::{AppError, AppResult};
use uuid::Uuid;

use super::models::*;
use super::repo;
use crate::state::AppState;

/// POST /v1/orders — checkout the current cart.
pub async fn checkout(
    State(st): State<AppState>,
    user: AuthUser,
) -> AppResult<(StatusCode, Json<OrderResponse>)> {
    user.require("order:write")?;
    let user_id = user.0.user_id()?;
    let order = repo::checkout(&st.pool, user_id).await?;
    let items = repo::order_items(&st.pool, order.id).await?;
    Ok((
        StatusCode::CREATED,
        Json(OrderResponse::build(order, items)),
    ))
}

/// GET /v1/orders — keyset-paginated list of the user's orders.
pub async fn list_orders(
    State(st): State<AppState>,
    user: AuthUser,
    Query(page): Query<PageQuery>,
) -> AppResult<Json<Page<OrderSummary>>> {
    user.require("order:read")?;
    let user_id = user.0.user_id()?;
    let limit = page.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let cursor = match &page.cursor {
        Some(c) => Some(Cursor::decode(c)?),
        None => None,
    };

    let mut rows = repo::list_orders(&st.pool, user_id, limit, cursor).await?;
    let next = if rows.len() as i64 > limit {
        rows.truncate(limit as usize);
        rows.last().map(|o| {
            Cursor {
                ts: o.created_at.to_rfc3339(),
                id: o.id.to_string(),
            }
            .encode()
        })
    } else {
        None
    };

    let summaries = rows.into_iter().map(OrderSummary::from).collect();
    Ok(Json(Page::new(summaries, next)))
}

/// GET /v1/orders/:id
pub async fn get_order(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OrderResponse>> {
    user.require("order:read")?;
    let user_id = user.0.user_id()?;
    let order = repo::get_order(&st.pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::not_found("order"))?;
    let items = repo::order_items(&st.pool, order.id).await?;
    Ok(Json(OrderResponse::build(order, items)))
}

/// POST /v1/orders/:id/cancel
pub async fn cancel_order(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OrderResponse>> {
    user.require("order:write")?;
    let user_id = user.0.user_id()?;
    let order = repo::cancel(&st.pool, user_id, id).await?;
    let items = repo::order_items(&st.pool, order.id).await?;
    Ok(Json(OrderResponse::build(order, items)))
}
