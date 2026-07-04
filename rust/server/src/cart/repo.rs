use common::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::CartItemJoin;

pub async fn get_cart(pool: &PgPool, user_id: Uuid) -> Result<Vec<CartItemJoin>, AppError> {
    Ok(sqlx::query_as::<_, CartItemJoin>(
        "SELECT ci.product_id, p.name, p.price_cents, p.currency, ci.quantity \
         FROM cart_items ci JOIN products p ON p.id = ci.product_id \
         WHERE ci.user_id = $1 ORDER BY ci.added_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

/// Add `qty` of a product, creating or incrementing the line. 404 if the
/// product does not exist or is archived.
pub async fn add_item(
    pool: &PgPool,
    user_id: Uuid,
    product_id: Uuid,
    qty: i32,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM products WHERE id = $1 AND status = 'active')",
    )
    .bind(product_id)
    .fetch_one(pool)
    .await?;
    if !exists {
        return Err(AppError::not_found("product"));
    }

    sqlx::query(
        "INSERT INTO cart_items (user_id, product_id, quantity) VALUES ($1, $2, $3) \
         ON CONFLICT (user_id, product_id) DO UPDATE \
         SET quantity = cart_items.quantity + EXCLUDED.quantity",
    )
    .bind(user_id)
    .bind(product_id)
    .bind(qty)
    .execute(pool)
    .await?;
    Ok(())
}

/// Set an existing line to an absolute quantity. 404 if not in the cart.
pub async fn set_item(
    pool: &PgPool,
    user_id: Uuid,
    product_id: Uuid,
    qty: i32,
) -> Result<(), AppError> {
    let res =
        sqlx::query("UPDATE cart_items SET quantity = $3 WHERE user_id = $1 AND product_id = $2")
            .bind(user_id)
            .bind(product_id)
            .bind(qty)
            .execute(pool)
            .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("cart item"));
    }
    Ok(())
}

pub async fn remove_item(pool: &PgPool, user_id: Uuid, product_id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM cart_items WHERE user_id = $1 AND product_id = $2")
        .bind(user_id)
        .bind(product_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn clear(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM cart_items WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
