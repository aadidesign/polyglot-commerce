//! Product/category handlers. Reads are cache-aside; writes require
//! `catalog:write` (RBAC) and invalidate the cache.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use common::auth::AuthUser;
use common::pagination::{Cursor, Page, DEFAULT_LIMIT, MAX_LIMIT};
use common::{AppError, AppResult};
use uuid::Uuid;
use validator::Validate;

use super::models::*;
use super::{cache, repo};
use crate::state::AppState;

pub async fn list_products(
    State(st): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Page<Product>>> {
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let cursor = match &q.cursor {
        Some(c) => Some(Cursor::decode(c)?),
        None => None,
    };

    let mut rows = repo::list_products(&st.pool, &q, limit, cursor).await?;
    let next = if rows.len() as i64 > limit {
        rows.truncate(limit as usize);
        rows.last().map(|p| {
            Cursor {
                ts: p.created_at.to_rfc3339(),
                id: p.id.to_string(),
            }
            .encode()
        })
    } else {
        None
    };
    Ok(Json(Page::new(rows, next)))
}

pub async fn get_product(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Product>> {
    if let Some(p) = cache::get(&st.redis, id).await {
        return Ok(Json(p));
    }
    let p = repo::get_product(&st.pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("product"))?;
    cache::put(&st.redis, &p, st.cache_ttl_secs).await;
    Ok(Json(p))
}

pub async fn create_product(
    State(st): State<AppState>,
    user: AuthUser,
    Json(body): Json<CreateProduct>,
) -> AppResult<(StatusCode, Json<Product>)> {
    user.require("catalog:write")?;
    body.validate()?;
    let product = repo::create_product(&st.pool, &body).await?;
    Ok((StatusCode::CREATED, Json(product)))
}

pub async fn update_product(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProduct>,
) -> AppResult<Json<Product>> {
    user.require("catalog:write")?;
    body.validate()?;
    let product = repo::update_product(&st.pool, id, &body)
        .await?
        .ok_or_else(|| AppError::not_found("product"))?;
    cache::invalidate(&st.redis, id).await;
    Ok(Json(product))
}

pub async fn archive_product(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    user.require("catalog:write")?;
    if !repo::archive_product(&st.pool, id).await? {
        return Err(AppError::not_found("product"));
    }
    cache::invalidate(&st.redis, id).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_categories(State(st): State<AppState>) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(repo::list_categories(&st.pool).await?))
}

pub async fn create_category(
    State(st): State<AppState>,
    user: AuthUser,
    Json(body): Json<CreateCategory>,
) -> AppResult<(StatusCode, Json<Category>)> {
    user.require("catalog:write")?;
    body.validate()?;
    let category = repo::create_category(&st.pool, &body).await?;
    Ok((StatusCode::CREATED, Json(category)))
}
