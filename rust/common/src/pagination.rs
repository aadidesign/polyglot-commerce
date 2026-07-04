//! Opaque **cursor** pagination.
//!
//! We never use SQL `OFFSET` on hot read paths (it degrades linearly and is
//! unstable under concurrent writes). Instead the cursor encodes the last seen
//! sort key + id; the next page is a keyset predicate `(created_at, id) < (..)`.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_LIMIT: i64 = 20;
pub const MAX_LIMIT: i64 = 100;

#[derive(Debug, Clone, Deserialize)]
pub struct PageQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

impl PageQuery {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

/// The decoded keyset position carried by a cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor {
    /// RFC3339 timestamp of the last item on the previous page.
    pub ts: String,
    /// Tie-breaking id of the last item.
    pub id: String,
}

impl Cursor {
    pub fn encode(&self) -> String {
        let json = serde_json::to_vec(self).expect("cursor serialize");
        URL_SAFE_NO_PAD.encode(json)
    }

    pub fn decode(raw: &str) -> Result<Self, AppError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(raw)
            .map_err(|_| AppError::bad_request("invalid cursor"))?;
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("invalid cursor"))
    }
}

/// Envelope returned to clients for any paginated collection.
#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, next_cursor: Option<String>) -> Self {
        let has_more = next_cursor.is_some();
        Self {
            items,
            next_cursor,
            has_more,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_encode_decode_round_trip() {
        let c = Cursor {
            ts: "2026-05-31T12:00:00+00:00".to_string(),
            id: "11111111-1111-1111-1111-111111111111".to_string(),
        };
        let encoded = c.encode();
        let decoded = Cursor::decode(&encoded).unwrap();
        assert_eq!(decoded.ts, c.ts);
        assert_eq!(decoded.id, c.id);
    }

    #[test]
    fn rejects_garbage_cursor() {
        assert!(Cursor::decode("not-base64-!!!").is_err());
    }

    #[test]
    fn limit_is_clamped() {
        let q = PageQuery {
            limit: Some(9999),
            cursor: None,
        };
        assert_eq!(q.limit(), MAX_LIMIT);
        let q = PageQuery {
            limit: None,
            cursor: None,
        };
        assert_eq!(q.limit(), DEFAULT_LIMIT);
    }
}
