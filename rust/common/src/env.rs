//! Tiny, dependency-light environment configuration helpers (12-Factor).
//!
//! We deliberately avoid a heavyweight config crate: every service declares a
//! typed config struct and builds it from these helpers, so misconfiguration
//! fails fast at startup with a clear message.

use std::str::FromStr;

/// Load a `.env` file if present (development convenience; no-op in prod).
pub fn load_dotenv() {
    let _ = dotenvy::dotenv();
}

/// Required variable - returns an error naming the key if missing.
pub fn required(key: &str) -> anyhow::Result<String> {
    std::env::var(key).map_err(|_| anyhow::anyhow!("missing required env var: {key}"))
}

/// Optional variable with a default.
pub fn or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Parse an optional variable into `T`, falling back to `default` on
/// absence or parse failure.
pub fn parse_or<T: FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}

/// The current deployment environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Environment {
    pub fn from_env() -> Self {
        match or_default("ENVIRONMENT", "development")
            .to_lowercase()
            .as_str()
        {
            "production" | "prod" => Environment::Production,
            "staging" | "stage" => Environment::Staging,
            _ => Environment::Development,
        }
    }

    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }
}
