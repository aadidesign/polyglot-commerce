//! Catalog data access. Listing uses keyset (cursor) pagination plus optional
//! full-text search via a dynamically composed query.

use chrono::{DateTime, Utc};
use common::pagination::Cursor;
use common::AppError;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use super::models::*;

const COLS: &str = "id, sku, name, slug, description, price_cents, currency, \
                    stock_quantity, category_id, status, created_at, updated_at";

pub async fn list_products(
    pool: &PgPool,
    q: &ListQuery,
    limit: i64,
    cursor: Option<Cursor>,
) -> Result<Vec<Product>, AppError> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(format!(
        "SELECT {COLS} FROM products WHERE status = 'active'"
    ));

    if let Some(cat) = q.category_id {
        qb.push(" AND category_id = ").push_bind(cat);
    }
    if let Some(term) = q.q.as_ref().filter(|t| !t.trim().is_empty()) {
        qb.push(" AND search_vector @@ websearch_to_tsquery('english', ")
            .push_bind(term.clone())
            .push(")");
    }
    if let Some(c) = cursor {
        let ts: DateTime<Utc> =
            c.ts.parse()
                .map_err(|_| AppError::bad_request("invalid cursor"))?;
        let id: Uuid =
            c.id.parse()
                .map_err(|_| AppError::bad_request("invalid cursor"))?;
        qb.push(" AND (created_at, id) < (")
            .push_bind(ts)
            .push(", ")
            .push_bind(id)
            .push(")");
    }

    qb.push(" ORDER BY created_at DESC, id DESC LIMIT ")
        .push_bind(limit + 1);

    Ok(qb.build_query_as::<Product>().fetch_all(pool).await?)
}

pub async fn get_product(pool: &PgPool, id: Uuid) -> Result<Option<Product>, AppError> {
    Ok(
        sqlx::query_as::<_, Product>(&format!("SELECT {COLS} FROM products WHERE id = $1"))
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn create_product(pool: &PgPool, p: &CreateProduct) -> Result<Product, AppError> {
    Ok(sqlx::query_as::<_, Product>(&format!(
        "INSERT INTO products (sku, name, slug, description, price_cents, currency, stock_quantity, category_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {COLS}"
    ))
    .bind(&p.sku)
    .bind(&p.name)
    .bind(&p.slug)
    .bind(p.description.clone().unwrap_or_default())
    .bind(p.price_cents)
    .bind(p.currency.clone().unwrap_or_else(|| "USD".to_string()))
    .bind(p.stock_quantity.unwrap_or(0))
    .bind(p.category_id)
    .fetch_one(pool)
    .await?)
}

pub async fn update_product(
    pool: &PgPool,
    id: Uuid,
    u: &UpdateProduct,
) -> Result<Option<Product>, AppError> {
    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("UPDATE products SET updated_at = now()");
    if let Some(name) = &u.name {
        qb.push(", name = ").push_bind(name.clone());
    }
    if let Some(desc) = &u.description {
        qb.push(", description = ").push_bind(desc.clone());
    }
    if let Some(price) = u.price_cents {
        qb.push(", price_cents = ").push_bind(price);
    }
    if let Some(stock) = u.stock_quantity {
        qb.push(", stock_quantity = ").push_bind(stock);
    }
    if let Some(cat) = u.category_id {
        qb.push(", category_id = ").push_bind(cat);
    }
    if let Some(status) = &u.status {
        qb.push(", status = ").push_bind(status.clone());
    }
    qb.push(" WHERE id = ").push_bind(id);
    qb.push(format!(" RETURNING {COLS}"));

    Ok(qb.build_query_as::<Product>().fetch_optional(pool).await?)
}

pub async fn archive_product(pool: &PgPool, id: Uuid) -> Result<bool, AppError> {
    let res =
        sqlx::query("UPDATE products SET status = 'archived', updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn list_categories(pool: &PgPool) -> Result<Vec<Category>, AppError> {
    Ok(sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, parent_id, created_at FROM categories ORDER BY name",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn create_category(pool: &PgPool, c: &CreateCategory) -> Result<Category, AppError> {
    Ok(sqlx::query_as::<_, Category>(
        "INSERT INTO categories (name, slug, parent_id) VALUES ($1, $2, $3) \
         RETURNING id, name, slug, parent_id, created_at",
    )
    .bind(&c.name)
    .bind(&c.slug)
    .bind(c.parent_id)
    .fetch_one(pool)
    .await?)
}
