use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// A cart row joined with its product for display.
#[derive(Debug, FromRow)]
pub struct CartItemJoin {
    pub product_id: Uuid,
    pub name: String,
    pub price_cents: i64,
    pub currency: String,
    pub quantity: i32,
}

#[derive(Debug, Serialize)]
pub struct CartItem {
    pub product_id: Uuid,
    pub name: String,
    pub unit_price_cents: i64,
    pub quantity: i32,
    pub line_total_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct CartResponse {
    pub items: Vec<CartItem>,
    pub total_cents: i64,
    pub currency: String,
}

impl CartResponse {
    pub fn from_rows(rows: Vec<CartItemJoin>) -> Self {
        let currency = rows
            .first()
            .map(|r| r.currency.clone())
            .unwrap_or_else(|| "USD".to_string());
        let mut total = 0i64;
        let items = rows
            .into_iter()
            .map(|r| {
                let line = r.price_cents * r.quantity as i64;
                total += line;
                CartItem {
                    product_id: r.product_id,
                    name: r.name,
                    unit_price_cents: r.price_cents,
                    quantity: r.quantity,
                    line_total_cents: line,
                }
            })
            .collect();
        Self {
            items,
            total_cents: total,
            currency,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddItemRequest {
    pub product_id: Uuid,
    #[validate(range(min = 1, max = 1000))]
    pub quantity: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateItemRequest {
    #[validate(range(min = 1, max = 1000))]
    pub quantity: i32,
}
