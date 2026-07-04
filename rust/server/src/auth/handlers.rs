//! Auth HTTP handlers: registration, login, refresh-token rotation with reuse
//! detection (see common::auth + ADR 0004 in the repo root docs), logout, me.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use common::auth::AuthUser;
use common::{AppError, AppResult};
use uuid::Uuid;
use validator::Validate;

use super::models::*;
use super::repo;
use crate::state::AppState;

fn ts(secs: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(secs, 0).unwrap_or_else(Utc::now)
}

/// Mint a fresh access+refresh pair, persisting the refresh token for rotation.
async fn issue_session(
    st: &AppState,
    user_id: Uuid,
    family: Uuid,
    conn: &mut sqlx::PgConnection,
) -> AppResult<TokenResponse> {
    let roles = repo::roles(&mut *conn, user_id).await?;
    let perms = repo::permissions(&mut *conn, user_id).await?;

    let access = st.jwt.issue_access(user_id, roles, perms)?;
    let refresh = st.jwt.issue_refresh(user_id, family)?;

    let jti = Uuid::parse_str(&refresh.jti).map_err(|e| AppError::Internal(e.into()))?;
    repo::store_refresh(&mut *conn, jti, family, user_id, ts(refresh.expires_at)).await?;

    Ok(TokenResponse {
        access_token: access.token,
        refresh_token: refresh.token,
        token_type: "Bearer",
        expires_in: access.expires_at - Utc::now().timestamp(),
    })
}

pub async fn register(
    State(st): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<RegisterResponse>)> {
    req.validate()?;
    let hash = common::auth::hash_password(&req.password)?;

    let mut tx = st.pool.begin().await?;
    let user = repo::create_user(&mut *tx, &req.email, &hash, &req.full_name).await?;
    repo::assign_role(&mut *tx, user.id, "customer").await?;
    let tokens = issue_session(&st, user.id, Uuid::new_v4(), &mut tx).await?;
    tx.commit().await?;

    let roles = repo::roles(&st.pool, user.id).await?;
    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            user: UserResponse::from_row(user, roles),
            tokens,
        }),
    ))
}

pub async fn login(
    State(st): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<TokenResponse>> {
    req.validate()?;
    let user = repo::find_by_email(&st.pool, &req.email)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !common::auth::verify_password(&req.password, &user.password_hash) {
        return Err(AppError::Unauthorized);
    }
    if user.status != "active" {
        return Err(AppError::Forbidden);
    }

    let mut conn = st.pool.acquire().await?;
    let tokens = issue_session(&st, user.id, Uuid::new_v4(), &mut conn).await?;
    Ok(Json(tokens))
}

pub async fn refresh(
    State(st): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    let claims = st.jwt.verify_refresh(&req.refresh_token)?;
    let jti = Uuid::parse_str(&claims.jti).map_err(|_| AppError::Unauthorized)?;
    let family = Uuid::parse_str(&claims.family).map_err(|_| AppError::Unauthorized)?;

    let row = repo::get_refresh(&st.pool, jti)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if row.revoked {
        return Err(AppError::Unauthorized);
    }
    if row.used {
        tracing::warn!(%family, "refresh token reuse detected; revoking family");
        repo::revoke_family(&st.pool, family).await?;
        return Err(AppError::Unauthorized);
    }

    let mut tx = st.pool.begin().await?;
    repo::mark_refresh_used(&mut *tx, jti).await?;
    let tokens = issue_session(&st, row.user_id, family, &mut tx).await?;
    tx.commit().await?;
    Ok(Json(tokens))
}

pub async fn logout(State(st): State<AppState>, Json(req): Json<LogoutRequest>) -> StatusCode {
    if let Ok(claims) = st.jwt.verify_refresh(&req.refresh_token) {
        if let Ok(family) = Uuid::parse_str(&claims.family) {
            let _ = repo::revoke_family(&st.pool, family).await;
        }
    }
    StatusCode::NO_CONTENT
}

pub async fn me(
    State(st): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<UserResponse>> {
    let user_id = claims.user_id()?;
    let user = repo::find_by_id(&st.pool, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("user"))?;
    let roles = repo::roles(&st.pool, user_id).await?;
    Ok(Json(UserResponse::from_row(user, roles)))
}
