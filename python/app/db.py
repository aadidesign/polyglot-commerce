"""Postgres connection pool + a lightweight embedded-SQL migrator."""
from __future__ import annotations

import logging
import pathlib

import asyncpg

log = logging.getLogger("ecommerce")

MIGRATIONS_DIR = pathlib.Path(__file__).resolve().parent.parent / "migrations"


async def create_pool(dsn: str) -> asyncpg.Pool:
    return await asyncpg.create_pool(dsn, min_size=2, max_size=20, command_timeout=30)


async def migrate(pool: asyncpg.Pool) -> None:
    async with pool.acquire() as conn:
        await conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations ("
            "version TEXT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT now())"
        )
        for path in sorted(MIGRATIONS_DIR.glob("*.sql")):
            name = path.name
            exists = await conn.fetchval(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = $1)", name
            )
            if exists:
                continue
            sql = path.read_text(encoding="utf-8")
            async with conn.transaction():
                await conn.execute(sql)
                await conn.execute(
                    "INSERT INTO schema_migrations (version) VALUES ($1)", name
                )
            log.info("applied migration %s", name)
