//! `common` - internal infrastructure crate for the Rust e-commerce backend.
//!
//! It deliberately contains no business logic; it is the small framework the
//! application (`server`) builds on, so the domain code stays focused.
//!
//! Modules:
//! - [`env`]        typed 12-Factor configuration helpers
//! - [`error`]      `AppError` + RFC 7807 problem responses
//! - [`telemetry`]  structured logging + Prometheus RED metrics
//! - [`auth`]       Argon2id hashing, JWT access/refresh, RBAC extractor
//! - [`db`]         Postgres pool construction + health ping
//! - [`http`]       standard middleware stack + graceful-shutdown server
//! - [`pagination`] opaque keyset (cursor) pagination

pub mod auth;
pub mod db;
pub mod env;
pub mod error;
pub mod http;
pub mod pagination;
pub mod telemetry;

pub use error::{AppError, AppResult};
