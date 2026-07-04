"""FastAPI application factory: lifespan (db/redis), middleware, routers."""
from __future__ import annotations

import logging
from contextlib import asynccontextmanager

import redis.asyncio as aioredis
from fastapi import FastAPI, Response
from fastapi.middleware.cors import CORSMiddleware
from prometheus_client import CONTENT_TYPE_LATEST, generate_latest

from app import db
from app.config import load_settings
from app.errors import register_handlers
from app.observability import ObservabilityMiddleware, setup_logging
from app.routers import auth, cart, orders, products
from app.security import JWTManager

log = logging.getLogger("ecommerce")


@asynccontextmanager
async def lifespan(app: FastAPI):
    settings = load_settings()
    setup_logging(settings.log_format)
    app.state.settings = settings
    app.state.jwt = JWTManager(settings)

    app.state.pool = await db.create_pool(settings.database_url)
    await db.migrate(app.state.pool)
    log.info("migrations applied")

    app.state.redis = None
    if settings.redis_url:
        try:
            client = aioredis.from_url(settings.redis_url, decode_responses=False)
            await client.ping()
            app.state.redis = client
            log.info("connected to Redis")
        except Exception as exc:  # noqa: BLE001
            log.warning("Redis unavailable; caching disabled: %s", exc)

    yield

    await app.state.pool.close()
    if app.state.redis is not None:
        await app.state.redis.aclose()


def create_app() -> FastAPI:
    app = FastAPI(title="E-Commerce Backend (Python)", version="0.1.0", lifespan=lifespan)

    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_methods=["*"],
        allow_headers=["*"],
    )
    app.add_middleware(ObservabilityMiddleware)
    register_handlers(app)

    app.include_router(auth.router)
    app.include_router(products.router)
    app.include_router(cart.router)
    app.include_router(orders.router)

    @app.get("/healthz", include_in_schema=False)
    async def healthz() -> Response:
        return Response("ok")

    @app.get("/readyz", include_in_schema=False)
    async def readyz() -> Response:
        try:
            await app.state.pool.fetchval("SELECT 1")
            return Response("ready")
        except Exception:  # noqa: BLE001
            return Response("degraded", status_code=503)

    @app.get("/metrics", include_in_schema=False)
    async def metrics() -> Response:
        return Response(generate_latest(), media_type=CONTENT_TYPE_LATEST)

    return app


app = create_app()
