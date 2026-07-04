"""Typed application configuration loaded from the environment (12-Factor)."""
from __future__ import annotations

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", extra="ignore", case_sensitive=False)

    port: int = 8080
    database_url: str
    redis_url: str | None = None

    jwt_access_secret: str
    jwt_refresh_secret: str = "refresh-secret"
    jwt_access_ttl_secs: int = 900
    jwt_refresh_ttl_secs: int = 2_592_000
    jwt_issuer: str = "ecommerce.python"
    jwt_audience: str = "ecommerce.api"

    cache_ttl_secs: int = 60
    log_format: str = "json"


def load_settings() -> Settings:
    return Settings()  # type: ignore[call-arg]
