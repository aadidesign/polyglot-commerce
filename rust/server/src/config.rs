use common::auth::JwtConfig;
use common::db::DbConfig;

pub struct Config {
    pub port: u16,
    pub db: DbConfig,
    pub jwt: JwtConfig,
    pub redis_url: Option<String>,
    pub cache_ttl_secs: u64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let redis_url = std::env::var("REDIS_URL").ok().filter(|s| !s.is_empty());
        Ok(Self {
            port: common::env::parse_or("PORT", 8080),
            db: DbConfig::from_env()?,
            jwt: JwtConfig::from_env()?,
            redis_url,
            cache_ttl_secs: common::env::parse_or("CACHE_TTL_SECS", 60),
        })
    }
}
