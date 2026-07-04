//! Order data access. Checkout is a single ACID transaction that locks the
//! relevant product rows (`SELECT ... FOR UPDATE`), verifies stock, decrements
//! it, snapshots line prices, and clears the cart — so there is no oversell and
//! no need for a distributed saga. Cancellation compensates by restocking.

use chrono::{DateTime, Utc};
use common::pagination::Cursor;
use common::AppError;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::models::{OrderItemRow, OrderRow};

const ORDER_COLS: &str = "id, user_id, status, total_cents, currency, created_at, updated_at";

#[derive(FromRow)]
struct CheckoutRow {
    product_id: Uuid,
    name: String,
    price_cents: i64,
    currency: String,
    stock_quantity: i32,
    quantity: i32,
}

/// Place an order from the user's current cart. Mock payment is assumed to
/// succeed, so the order is created in `confirmed` status.
pub async fn checkout(pool: &PgPool, user_id: Uuid) -> Result<OrderRow, AppError> {
    let mut tx = pool.begin().await?;

    // Lock the product rows referenced by the cart for the duration of the tx.
    let cart: Vec<CheckoutRow> = sqlx::query_as(
        "SELECT ci.product_id, p.name, p.price_cents, p.currency, p.stock_quantity, ci.quantity \
         FROM cart_items ci JOIN products p ON p.id = ci.product_id \
         WHERE ci.user_id = $1 FOR UPDATE OF p",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;

    if cart.is_empty() {
        return Err(AppError::bad_request("cart is empty"));
    }

    let mut total: i64 = 0;
    for item in &cart {
        if item.quantity > item.stock_quantity {
            return Err(AppError::conflict(format!(
                "insufficient stock for '{}' (requested {}, available {})",
                item.name, item.quantity, item.stock_quantity
            )));
        }
        total += item.price_cents * item.quantity as i64;
    }
    let currency = cart[0].currency.clone();

    let order: OrderRow = sqlx::query_as(&format!(
        "INSERT INTO orders (user_id, status, total_cents, currency) \
         VALUES ($1, 'confirmed', $2, $3) RETURNING {ORDER_COLS}"
    ))
    .bind(user_id)
    .bind(total)
    .bind(&currency)
    .fetch_one(&mut *tx)
    .await?;

    for item in &cart {
        sqlx::query(
            "INSERT INTO order_items (order_id, product_id, product_name, unit_price_cents, quantity) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(order.id)
        .bind(item.product_id)
        .bind(&item.name)
        .bind(item.price_cents)
        .bind(item.quantity)
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE products SET stock_quantity = stock_quantity - $2 WHERE id = $1")
            .bind(item.product_id)
            .bind(item.quantity)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("DELETE FROM cart_items WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(order)
}

pub async fn list_orders(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
    cursor: Option<Cursor>,
) -> Result<Vec<OrderRow>, AppError> {
    let rows = if let Some(c) = cursor {
        let ts: DateTime<Utc> =
            c.ts.parse()
                .map_err(|_| AppError::bad_request("invalid cursor"))?;
        let id: Uuid =
            c.id.parse()
                .map_err(|_| AppError::bad_request("invalid cursor"))?;
        sqlx::query_as::<_, OrderRow>(&format!(
            "SELECT {ORDER_COLS} FROM orders WHERE user_id = $1 AND (created_at, id) < ($2, $3) \
             ORDER BY created_at DESC, id DESC LIMIT $4"
        ))
        .bind(user_id)
        .bind(ts)
        .bind(id)
        .bind(limit + 1)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, OrderRow>(&format!(
            "SELECT {ORDER_COLS} FROM orders WHERE user_id = $1 \
             ORDER BY created_at DESC, id DESC LIMIT $2"
        ))
        .bind(user_id)
        .bind(limit + 1)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

pub async fn get_order(
    pool: &PgPool,
    user_id: Uuid,
    order_id: Uuid,
) -> Result<Option<OrderRow>, AppError> {
    Ok(sqlx::query_as::<_, OrderRow>(&format!(
        "SELECT {ORDER_COLS} FROM orders WHERE id = $1 AND user_id = $2"
    ))
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn order_items(pool: &PgPool, order_id: Uuid) -> Result<Vec<OrderItemRow>, AppError> {
    Ok(sqlx::query_as::<_, OrderItemRow>(
        "SELECT product_id, product_name, unit_price_cents, quantity \
         FROM order_items WHERE order_id = $1",
    )
    .bind(order_id)
    .fetch_all(pool)
    .await?)
}

/// Cancel an order and restock its items. Idempotent-ish: cancelling an
/// already-cancelled order is a conflict.
pub async fn cancel(pool: &PgPool, user_id: Uuid, order_id: Uuid) -> Result<OrderRow, AppError> {
    let mut tx = pool.begin().await?;

    let order: Option<OrderRow> = sqlx::query_as(&format!(
        "SELECT {ORDER_COLS} FROM orders WHERE id = $1 AND user_id = $2 FOR UPDATE"
    ))
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let order = order.ok_or_else(|| AppError::not_found("order"))?;
    if order.status == "cancelled" {
        return Err(AppError::conflict("order is already cancelled"));
    }

    let items: Vec<OrderItemRow> = sqlx::query_as(
        "SELECT product_id, product_name, unit_price_cents, quantity FROM order_items WHERE order_id = $1",
    )
    .bind(order_id)
    .fetch_all(&mut *tx)
    .await?;

    for item in &items {
        sqlx::query("UPDATE products SET stock_quantity = stock_quantity + $2 WHERE id = $1")
            .bind(item.product_id)
            .bind(item.quantity)
            .execute(&mut *tx)
            .await?;
    }

    let updated: OrderRow = sqlx::query_as(&format!(
        "UPDATE orders SET status = 'cancelled', updated_at = now() WHERE id = $1 RETURNING {ORDER_COLS}"
    ))
    .bind(order_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(updated)
}
