use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)] // full row is mapped by sqlx; not every column is read in code
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub full_name: String,
    pub email_verified: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct RefreshRow {
    pub jti: Uuid,
    pub family: Uuid,
    pub user_id: Uuid,
    pub used: bool,
    pub revoked: bool,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 chars"))]
    pub password: String,
    #[validate(length(min = 1, max = 200))]
    pub full_name: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub email_verified: bool,
    pub status: String,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl UserResponse {
    pub fn from_row(u: UserRow, roles: Vec<String>) -> Self {
        Self {
            id: u.id,
            email: u.email,
            full_name: u.full_name,
            email_verified: u.email_verified,
            status: u.status,
            roles,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user: UserResponse,
    pub tokens: TokenResponse,
}
