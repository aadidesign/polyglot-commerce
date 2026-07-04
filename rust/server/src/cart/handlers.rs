//! Cart handlers. Every route requires authentication; the cart is owned by the
//! authenticated user (derived from the access token, never from the request).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use common::auth::AuthUser;
use common::AppResult;
use uuid::Uuid;
use validator::Validate;

use super::models::*;
use super::repo;
use crate::state::AppState;

pub async fn get_cart(
    State(st): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<CartResponse>> {
    let user_id = claims.user_id()?;
    let rows = repo::get_cart(&st.pool, user_id).await?;
    Ok(Json(CartResponse::from_rows(rows)))
}

pub async fn add_item(
    State(st): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<AddItemRequest>,
) -> AppResult<Json<CartResponse>> {
    body.validate()?;
    let user_id = claims.user_id()?;
    repo::add_item(&st.pool, user_id, body.product_id, body.quantity).await?;
    let rows = repo::get_cart(&st.pool, user_id).await?;
    Ok(Json(CartResponse::from_rows(rows)))
}

pub async fn update_item(
    State(st): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(product_id): Path<Uuid>,
    Json(body): Json<UpdateItemRequest>,
) -> AppResult<Json<CartResponse>> {
    body.validate()?;
    let user_id = claims.user_id()?;
    repo::set_item(&st.pool, user_id, product_id, body.quantity).await?;
    let rows = repo::get_cart(&st.pool, user_id).await?;
    Ok(Json(CartResponse::from_rows(rows)))
}

pub async fn remove_item(
    State(st): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(product_id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user_id = claims.user_id()?;
    repo::remove_item(&st.pool, user_id, product_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn clear_cart(
    State(st): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<StatusCode> {
    let user_id = claims.user_id()?;
    repo::clear(&st.pool, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
