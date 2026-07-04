//! Auth data access. Queries are generic over any sqlx executor so they work
//! with both a pool and a transaction.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use super::models::{RefreshRow, UserRow};

const USER_COLS: &str =
    "id, email, password_hash, full_name, email_verified, status, created_at, updated_at";

pub async fn create_user<'e, E>(
    exec: E,
    email: &str,
    password_hash: &str,
    full_name: &str,
) -> Result<UserRow, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, UserRow>(&format!(
        "INSERT INTO users (email, password_hash, full_name) \
         VALUES ($1, $2, $3) RETURNING {USER_COLS}"
    ))
    .bind(email)
    .bind(password_hash)
    .bind(full_name)
    .fetch_one(exec)
    .await
}

pub async fn assign_role<'e, E>(exec: E, user_id: Uuid, role: &str) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO user_roles (user_id, role_id) \
         SELECT $1, id FROM roles WHERE name = $2 ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(role)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn find_by_email<'e, E>(exec: E, email: &str) -> Result<Option<UserRow>, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, UserRow>(&format!("SELECT {USER_COLS} FROM users WHERE email = $1"))
        .bind(email)
        .fetch_optional(exec)
        .await
}

pub async fn find_by_id<'e, E>(exec: E, id: Uuid) -> Result<Option<UserRow>, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, UserRow>(&format!("SELECT {USER_COLS} FROM users WHERE id = $1"))
        .bind(id)
        .fetch_optional(exec)
        .await
}

pub async fn roles<'e, E>(exec: E, user_id: Uuid) -> Result<Vec<String>, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, String>(
        "SELECT r.name FROM roles r \
         JOIN user_roles ur ON ur.role_id = r.id WHERE ur.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(exec)
    .await
}

pub async fn permissions<'e, E>(exec: E, user_id: Uuid) -> Result<Vec<String>, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT p.name FROM permissions p \
         JOIN role_permissions rp ON rp.permission_id = p.id \
         JOIN user_roles ur ON ur.role_id = rp.role_id \
         WHERE ur.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(exec)
    .await
}

pub async fn store_refresh<'e, E>(
    exec: E,
    jti: Uuid,
    family: Uuid,
    user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, family, user_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(jti)
    .bind(family)
    .bind(user_id)
    .bind(expires_at)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn get_refresh<'e, E>(exec: E, jti: Uuid) -> Result<Option<RefreshRow>, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, RefreshRow>(
        "SELECT jti, family, user_id, used, revoked, expires_at FROM refresh_tokens WHERE jti = $1",
    )
    .bind(jti)
    .fetch_optional(exec)
    .await
}

pub async fn mark_refresh_used<'e, E>(exec: E, jti: Uuid) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("UPDATE refresh_tokens SET used = TRUE WHERE jti = $1")
        .bind(jti)
        .execute(exec)
        .await?;
    Ok(())
}

/// Revoke every token in a family - the response to refresh-token reuse.
pub async fn revoke_family<'e, E>(exec: E, family: Uuid) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE family = $1")
        .bind(family)
        .execute(exec)
        .await?;
    Ok(())
}
