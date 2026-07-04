"""Reusable FastAPI dependencies: DB pool, JWT manager, auth + RBAC guards."""
from __future__ import annotations

import uuid

import asyncpg
import jwt
from fastapi import Depends, Header, Request

from app.errors import forbidden, unauthorized
from app.security import JWTManager


def get_pool(request: Request) -> asyncpg.Pool:
    return request.app.state.pool


def get_jwt(request: Request) -> JWTManager:
    return request.app.state.jwt


def get_settings(request: Request):
    return request.app.state.settings


def get_redis(request: Request):
    return getattr(request.app.state, "redis", None)


async def current_claims(
    authorization: str | None = Header(default=None),
    jwt_mgr: JWTManager = Depends(get_jwt),
) -> dict:
    if not authorization or not authorization.startswith("Bearer "):
        raise unauthorized()
    try:
        return jwt_mgr.verify_access(authorization[len("Bearer "):])
    except jwt.PyJWTError as exc:
        raise unauthorized() from exc


def current_user_id(claims: dict = Depends(current_claims)) -> uuid.UUID:
    try:
        return uuid.UUID(claims["sub"])
    except (KeyError, ValueError) as exc:
        raise unauthorized() from exc


def require_permission(permission: str):
    """Dependency factory enforcing an RBAC permission."""

    async def _dep(claims: dict = Depends(current_claims)) -> dict:
        perms = claims.get("permissions", [])
        if permission not in perms and "*" not in perms:
            raise forbidden()
        return claims

    return _dep
