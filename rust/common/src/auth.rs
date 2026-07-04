//! Authentication & authorization primitives shared by all Rust services.
//!
//! - Argon2id password hashing/verification (OWASP-recommended, memory-hard).
//! - JWT issuance/verification for short-lived **access** tokens and rotating
//!   **refresh** tokens (see ADR 0004).
//! - An axum extractor (`AuthUser`) that any service exposing [`HasJwt`] can use
//!   to require authentication, plus permission guards for RBAC.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

// ----------------------------------------------------------------- passwords --

/// Hash a plaintext password with Argon2id and a fresh random salt.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let mut salt_bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut salt_bytes);
    let salt =
        SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow::anyhow!("salt encode: {e}"))?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash: {e}"))?;
    Ok(hash.to_string())
}

/// Verify a plaintext password against a stored PHC hash. Constant-time.
pub fn verify_password(password: &str, phc_hash: &str) -> bool {
    match PasswordHash::new(phc_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// --------------------------------------------------------------------- claims --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: String, // user id
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub iss: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
    pub typ: String, // "access"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: String,
    pub family: String, // token-family id for rotation/reuse detection
    pub iss: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
    pub typ: String, // "refresh"
}

impl AccessClaims {
    pub fn user_id(&self) -> Result<Uuid, AppError> {
        Uuid::parse_str(&self.sub).map_err(|_| AppError::Unauthorized)
    }
    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.iter().any(|p| p == perm || p == "*")
    }
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

// ------------------------------------------------------------------ jwt mgmt --

#[derive(Clone)]
pub struct JwtConfig {
    pub access_secret: String,
    pub refresh_secret: String,
    pub access_ttl_secs: i64,
    pub refresh_ttl_secs: i64,
    pub issuer: String,
    pub audience: String,
}

impl JwtConfig {
    /// Build from the standard environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            access_secret: crate::env::required("JWT_ACCESS_SECRET")?,
            refresh_secret: crate::env::or_default("JWT_REFRESH_SECRET", "refresh-secret"),
            access_ttl_secs: crate::env::parse_or("JWT_ACCESS_TTL_SECS", 900),
            refresh_ttl_secs: crate::env::parse_or("JWT_REFRESH_TTL_SECS", 2_592_000),
            issuer: crate::env::or_default("JWT_ISSUER", "ecommerce.identity"),
            audience: crate::env::or_default("JWT_AUDIENCE", "ecommerce.api"),
        })
    }
}

#[derive(Clone)]
pub struct JwtManager {
    cfg: JwtConfig,
    access_enc: EncodingKey,
    access_dec: DecodingKey,
    refresh_enc: EncodingKey,
    refresh_dec: DecodingKey,
    v_access: Validation,
    v_refresh: Validation,
}

/// A freshly issued token plus the metadata a caller needs to persist.
pub struct IssuedToken {
    pub token: String,
    pub jti: String,
    pub expires_at: i64,
}

impl JwtManager {
    pub fn new(cfg: JwtConfig) -> Self {
        let mut v_access = Validation::new(Algorithm::HS256);
        v_access.set_issuer(std::slice::from_ref(&cfg.issuer));
        v_access.set_audience(std::slice::from_ref(&cfg.audience));
        v_access.set_required_spec_claims(&["exp", "iss", "aud"]);

        let mut v_refresh = Validation::new(Algorithm::HS256);
        v_refresh.set_issuer(std::slice::from_ref(&cfg.issuer));
        v_refresh.set_audience(std::slice::from_ref(&cfg.audience));
        v_refresh.set_required_spec_claims(&["exp", "iss", "aud"]);

        Self {
            access_enc: EncodingKey::from_secret(cfg.access_secret.as_bytes()),
            access_dec: DecodingKey::from_secret(cfg.access_secret.as_bytes()),
            refresh_enc: EncodingKey::from_secret(cfg.refresh_secret.as_bytes()),
            refresh_dec: DecodingKey::from_secret(cfg.refresh_secret.as_bytes()),
            v_access,
            v_refresh,
            cfg,
        }
    }

    pub fn issue_access(
        &self,
        user_id: Uuid,
        roles: Vec<String>,
        permissions: Vec<String>,
    ) -> anyhow::Result<IssuedToken> {
        let now = Utc::now().timestamp();
        let exp = now + self.cfg.access_ttl_secs;
        let jti = Uuid::new_v4().to_string();
        let claims = AccessClaims {
            sub: user_id.to_string(),
            roles,
            permissions,
            iss: self.cfg.issuer.clone(),
            aud: self.cfg.audience.clone(),
            iat: now,
            exp,
            jti: jti.clone(),
            typ: "access".into(),
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.access_enc)?;
        Ok(IssuedToken {
            token,
            jti,
            expires_at: exp,
        })
    }

    pub fn issue_refresh(&self, user_id: Uuid, family: Uuid) -> anyhow::Result<IssuedToken> {
        let now = Utc::now().timestamp();
        let exp = now + self.cfg.refresh_ttl_secs;
        let jti = Uuid::new_v4().to_string();
        let claims = RefreshClaims {
            sub: user_id.to_string(),
            family: family.to_string(),
            iss: self.cfg.issuer.clone(),
            aud: self.cfg.audience.clone(),
            iat: now,
            exp,
            jti: jti.clone(),
            typ: "refresh".into(),
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.refresh_enc)?;
        Ok(IssuedToken {
            token,
            jti,
            expires_at: exp,
        })
    }

    pub fn verify_access(&self, token: &str) -> Result<AccessClaims, AppError> {
        let data = decode::<AccessClaims>(token, &self.access_dec, &self.v_access)
            .map_err(|_| AppError::Unauthorized)?;
        if data.claims.typ != "access" {
            return Err(AppError::Unauthorized);
        }
        Ok(data.claims)
    }

    pub fn verify_refresh(&self, token: &str) -> Result<RefreshClaims, AppError> {
        let data = decode::<RefreshClaims>(token, &self.refresh_dec, &self.v_refresh)
            .map_err(|_| AppError::Unauthorized)?;
        if data.claims.typ != "refresh" {
            return Err(AppError::Unauthorized);
        }
        Ok(data.claims)
    }
}

// ------------------------------------------------------------------ extractor --

/// Implemented by a service's `AppState` to expose its [`JwtManager`], enabling
/// the [`AuthUser`] extractor on its routes.
pub trait HasJwt {
    fn jwt(&self) -> &JwtManager;
}

/// Blanket impl so services can use `Arc<Inner>` directly as router state.
/// (Defined here because `HasJwt` is local to this crate, side-stepping the
/// orphan rule that would block the impl in a service crate.)
impl<T: HasJwt> HasJwt for std::sync::Arc<T> {
    fn jwt(&self) -> &JwtManager {
        (**self).jwt()
    }
}

/// Extractor that requires a valid bearer access token and yields its claims.
pub struct AuthUser(pub AccessClaims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: HasJwt + Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;
        let claims = state.jwt().verify_access(token)?;
        Ok(AuthUser(claims))
    }
}

impl AuthUser {
    /// Guard a route on a required permission (RBAC).
    pub fn require(&self, permission: &str) -> Result<(), AppError> {
        if self.0.has_permission(permission) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_round_trip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
        assert!(hash.starts_with("$argon2id$"));
    }

    fn test_manager() -> JwtManager {
        JwtManager::new(JwtConfig {
            access_secret: "test-access-secret-0123456789abcdef".into(),
            refresh_secret: "test-refresh-secret-0123456789abcd".into(),
            access_ttl_secs: 900,
            refresh_ttl_secs: 3600,
            issuer: "test.iss".into(),
            audience: "test.aud".into(),
        })
    }

    #[test]
    fn access_token_round_trip_and_rbac() {
        let mgr = test_manager();
        let uid = Uuid::new_v4();
        let issued = mgr
            .issue_access(uid, vec!["customer".into()], vec!["order:write".into()])
            .unwrap();

        let claims = mgr.verify_access(&issued.token).unwrap();
        assert_eq!(claims.user_id().unwrap(), uid);
        assert!(claims.has_permission("order:write"));
        assert!(!claims.has_permission("catalog:write"));
        assert!(claims.has_role("customer"));
    }

    #[test]
    fn access_verifier_rejects_refresh_token() {
        let mgr = test_manager();
        let uid = Uuid::new_v4();
        let refresh = mgr.issue_refresh(uid, Uuid::new_v4()).unwrap();
        assert!(mgr.verify_access(&refresh.token).is_err());
    }

    #[test]
    fn wildcard_permission_grants_everything() {
        let claims = AccessClaims {
            sub: Uuid::new_v4().to_string(),
            roles: vec!["admin".into()],
            permissions: vec!["*".into()],
            iss: "i".into(),
            aud: "a".into(),
            iat: 0,
            exp: 0,
            jti: "j".into(),
            typ: "access".into(),
        };
        assert!(claims.has_permission("anything:at:all"));
    }
}
