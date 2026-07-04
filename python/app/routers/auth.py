"""Auth endpoints: register, login, refresh (rotation + reuse detection), me."""
from __future__ import annotations

import uuid
from datetime import datetime, timezone

import asyncpg
import jwt
from fastapi import APIRouter, Depends

from app.deps import current_claims, get_jwt, get_pool
from app.errors import conflict, forbidden, not_found, unauthorized
from app.schemas import LoginRequest, LogoutRequest, RefreshRequest, RegisterRequest
from app.security import JWTManager, hash_password, verify_password

router = APIRouter(prefix="/v1/auth", tags=["auth"])


async def _roles(conn, user_id: uuid.UUID) -> list[str]:
    rows = await conn.fetch(
        "SELECT r.name FROM roles r JOIN user_roles ur ON ur.role_id = r.id WHERE ur.user_id = $1",
        user_id,
    )
    return [r["name"] for r in rows]


async def _perms(conn, user_id: uuid.UUID) -> list[str]:
    rows = await conn.fetch(
        "SELECT DISTINCT p.name FROM permissions p "
        "JOIN role_permissions rp ON rp.permission_id = p.id "
        "JOIN user_roles ur ON ur.role_id = rp.role_id WHERE ur.user_id = $1",
        user_id,
    )
    return [r["name"] for r in rows]


async def _issue_session(conn, jwt_mgr: JWTManager, user_id: uuid.UUID, family: uuid.UUID) -> dict:
    roles = await _roles(conn, user_id)
    perms = await _perms(conn, user_id)
    access = jwt_mgr.issue_access(user_id, roles, perms)
    refresh = jwt_mgr.issue_refresh(user_id, family)
    await conn.execute(
        "INSERT INTO refresh_tokens (jti, family, user_id, expires_at) VALUES ($1,$2,$3,$4)",
        refresh.jti, family, user_id, refresh.expires_at,
    )
    expires_in = int((access.expires_at - datetime.now(timezone.utc)).total_seconds())
    return {
        "access_token": access.token,
        "refresh_token": refresh.token,
        "token_type": "Bearer",
        "expires_in": expires_in,
    }


def _user_dict(row, roles: list[str]) -> dict:
    return {
        "id": str(row["id"]),
        "email": row["email"],
        "full_name": row["full_name"],
        "email_verified": row["email_verified"],
        "status": row["status"],
        "roles": roles,
        "created_at": row["created_at"].isoformat(),
    }


@router.post("/register", status_code=201)
async def register(body: RegisterRequest, pool=Depends(get_pool), jwt_mgr=Depends(get_jwt)):
    pw_hash = hash_password(body.password)
    async with pool.acquire() as conn:
        async with conn.transaction():
            try:
                row = await conn.fetchrow(
                    "INSERT INTO users (email, password_hash, full_name) VALUES ($1,$2,$3) "
                    "RETURNING id, email, full_name, email_verified, status, created_at",
                    body.email, pw_hash, body.full_name,
                )
            except asyncpg.UniqueViolationError:
                raise conflict("email already registered")
            await conn.execute(
                "INSERT INTO user_roles (user_id, role_id) "
                "SELECT $1, id FROM roles WHERE name = 'customer' ON CONFLICT DO NOTHING",
                row["id"],
            )
            tokens = await _issue_session(conn, jwt_mgr, row["id"], uuid.uuid4())
        roles = await _roles(conn, row["id"])
    return {"user": _user_dict(row, roles), "tokens": tokens}


@router.post("/login")
async def login(body: LoginRequest, pool=Depends(get_pool), jwt_mgr=Depends(get_jwt)):
    async with pool.acquire() as conn:
        row = await conn.fetchrow(
            "SELECT id, password_hash, status FROM users WHERE email = $1", body.email
        )
        if row is None or not verify_password(body.password, row["password_hash"]):
            raise unauthorized()
        if row["status"] != "active":
            raise forbidden()
        return await _issue_session(conn, jwt_mgr, row["id"], uuid.uuid4())


@router.post("/refresh")
async def refresh(body: RefreshRequest, pool=Depends(get_pool), jwt_mgr=Depends(get_jwt)):
    try:
        claims = jwt_mgr.verify_refresh(body.refresh_token)
    except jwt.PyJWTError:
        raise unauthorized()
    jti = uuid.UUID(claims["jti"])
    family = uuid.UUID(claims["family"])

    async with pool.acquire() as conn:
        rec = await conn.fetchrow(
            "SELECT user_id, used, revoked FROM refresh_tokens WHERE jti = $1", jti
        )
        if rec is None or rec["revoked"]:
            raise unauthorized()
        if rec["used"]:
            # Reuse detected - revoke the whole family.
            await conn.execute("UPDATE refresh_tokens SET revoked = TRUE WHERE family = $1", family)
            raise unauthorized()
        async with conn.transaction():
            await conn.execute("UPDATE refresh_tokens SET used = TRUE WHERE jti = $1", jti)
            return await _issue_session(conn, jwt_mgr, rec["user_id"], family)


@router.post("/logout", status_code=204)
async def logout(body: LogoutRequest, pool=Depends(get_pool), jwt_mgr=Depends(get_jwt)):
    try:
        claims = jwt_mgr.verify_refresh(body.refresh_token)
        family = uuid.UUID(claims["family"])
        async with pool.acquire() as conn:
            await conn.execute("UPDATE refresh_tokens SET revoked = TRUE WHERE family = $1", family)
    except Exception:  # noqa: BLE001 - idempotent logout never leaks token validity
        pass
    return None


@router.get("/me")
async def me(claims: dict = Depends(current_claims), pool=Depends(get_pool)):
    user_id = uuid.UUID(claims["sub"])
    async with pool.acquire() as conn:
        row = await conn.fetchrow(
            "SELECT id, email, full_name, email_verified, status, created_at "
            "FROM users WHERE id = $1",
            user_id,
        )
        if row is None:
            raise not_found("user")
        roles = await _roles(conn, user_id)
    return _user_dict(row, roles)
