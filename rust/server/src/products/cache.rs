//! Read-through product cache backed by Redis. All operations are best-effort:
//! a Redis outage degrades to database-only reads rather than failing requests.

use redis::aio::ConnectionManager;
use uuid::Uuid;

use super::models::Product;

fn key(id: Uuid) -> String {
    format!("catalog:product:{id}")
}

pub async fn get(redis: &Option<ConnectionManager>, id: Uuid) -> Option<Product> {
    let mut cm = redis.as_ref()?.clone();
    let raw: redis::RedisResult<Option<String>> =
        redis::cmd("GET").arg(key(id)).query_async(&mut cm).await;
    match raw {
        Ok(Some(s)) => serde_json::from_str(&s).ok(),
        _ => None,
    }
}

pub async fn put(redis: &Option<ConnectionManager>, product: &Product, ttl_secs: u64) {
    let Some(cm) = redis.as_ref() else { return };
    let mut cm = cm.clone();
    if let Ok(s) = serde_json::to_string(product) {
        let _: redis::RedisResult<()> = redis::cmd("SET")
            .arg(key(product.id))
            .arg(s)
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut cm)
            .await;
    }
}

pub async fn invalidate(redis: &Option<ConnectionManager>, id: Uuid) {
    let Some(cm) = redis.as_ref() else { return };
    let mut cm = cm.clone();
    let _: redis::RedisResult<i64> = redis::cmd("DEL").arg(key(id)).query_async(&mut cm).await;
}
