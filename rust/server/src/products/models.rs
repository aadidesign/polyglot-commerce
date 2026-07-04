use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub price_cents: i64,
    pub currency: String,
    pub stock_quantity: i32,
    pub category_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProduct {
    #[validate(length(min = 1, max = 64))]
    pub sku: String,
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    #[validate(length(min = 1, max = 200))]
    pub slug: String,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    #[validate(range(min = 0))]
    pub price_cents: i64,
    pub currency: Option<String>,
    #[validate(range(min = 0))]
    pub stock_quantity: Option<i32>,
    pub category_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProduct {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    #[validate(range(min = 0))]
    pub price_cents: Option<i64>,
    #[validate(range(min = 0))]
    pub stock_quantity: Option<i32>,
    pub category_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCategory {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(length(min = 1, max = 120))]
    pub slug: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub category_id: Option<Uuid>,
    /// Full-text search term (Postgres `websearch_to_tsquery`).
    pub q: Option<String>,
}
