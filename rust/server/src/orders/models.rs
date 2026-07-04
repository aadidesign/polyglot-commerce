use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)] // full row is mapped by sqlx; not every column is read in code
pub struct OrderRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub total_cents: i64,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct OrderItemRow {
    pub product_id: Uuid,
    pub product_name: String,
    pub unit_price_cents: i64,
    pub quantity: i32,
}

#[derive(Debug, Serialize)]
pub struct OrderItemResponse {
    pub product_id: Uuid,
    pub product_name: String,
    pub unit_price_cents: i64,
    pub quantity: i32,
    pub line_total_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: Uuid,
    pub status: String,
    pub total_cents: i64,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub items: Vec<OrderItemResponse>,
}

impl OrderResponse {
    pub fn build(order: OrderRow, items: Vec<OrderItemRow>) -> Self {
        Self {
            id: order.id,
            status: order.status,
            total_cents: order.total_cents,
            currency: order.currency,
            created_at: order.created_at,
            items: items
                .into_iter()
                .map(|i| OrderItemResponse {
                    line_total_cents: i.unit_price_cents * i.quantity as i64,
                    product_id: i.product_id,
                    product_name: i.product_name,
                    unit_price_cents: i.unit_price_cents,
                    quantity: i.quantity,
                })
                .collect(),
        }
    }
}

/// Compact representation for the order-list endpoint.
#[derive(Debug, Serialize)]
pub struct OrderSummary {
    pub id: Uuid,
    pub status: String,
    pub total_cents: i64,
    pub currency: String,
    pub created_at: DateTime<Utc>,
}

impl From<OrderRow> for OrderSummary {
    fn from(o: OrderRow) -> Self {
        Self {
            id: o.id,
            status: o.status,
            total_cents: o.total_cents,
            currency: o.currency,
            created_at: o.created_at,
        }
    }
}
