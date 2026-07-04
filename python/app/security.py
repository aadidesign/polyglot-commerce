"""Password hashing (Argon2id) and JWT issuance/verification."""
from __future__ import annotations

import time
import uuid
from datetime import datetime, timezone

import jwt
from argon2 import PasswordHasher

from app.config import Settings

_ph = PasswordHasher()


def hash_password(password: str) -> str:
    return _ph.hash(password)


def verify_password(password: str, encoded: str) -> bool:
    try:
        return _ph.verify(encoded, password)
    except Exception:
        return False


class Issued:
    def __init__(self, token: str, jti: uuid.UUID, expires_at: datetime):
        self.token = token
        self.jti = jti
        self.expires_at = expires_at


class JWTManager:
    def __init__(self, s: Settings):
        self._access_secret = s.jwt_access_secret
        self._refresh_secret = s.jwt_refresh_secret
        self._access_ttl = s.jwt_access_ttl_secs
        self._refresh_ttl = s.jwt_refresh_ttl_secs
        self._issuer = s.jwt_issuer
        self._audience = s.jwt_audience

    def issue_access(self, user_id: uuid.UUID, roles: list[str], perms: list[str]) -> Issued:
        now = int(time.time())
        exp = now + self._access_ttl
        jti = uuid.uuid4()
        payload = {
            "sub": str(user_id),
            "roles": roles,
            "permissions": perms,
            "typ": "access",
            "iss": self._issuer,
            "aud": self._audience,
            "iat": now,
            "exp": exp,
            "jti": str(jti),
        }
        token = jwt.encode(payload, self._access_secret, algorithm="HS256")
        return Issued(token, jti, datetime.fromtimestamp(exp, tz=timezone.utc))

    def issue_refresh(self, user_id: uuid.UUID, family: uuid.UUID) -> Issued:
        now = int(time.time())
        exp = now + self._refresh_ttl
        jti = uuid.uuid4()
        payload = {
            "sub": str(user_id),
            "family": str(family),
            "typ": "refresh",
            "iss": self._issuer,
            "aud": self._audience,
            "iat": now,
            "exp": exp,
            "jti": str(jti),
        }
        token = jwt.encode(payload, self._refresh_secret, algorithm="HS256")
        return Issued(token, jti, datetime.fromtimestamp(exp, tz=timezone.utc))

    def _decode(self, token: str, secret: str) -> dict:
        return jwt.decode(
            token,
            secret,
            algorithms=["HS256"],
            audience=self._audience,
            issuer=self._issuer,
            options={"require": ["exp", "iss", "aud"]},
        )

    def verify_access(self, token: str) -> dict:
        claims = self._decode(token, self._access_secret)
        if claims.get("typ") != "access":
            raise jwt.InvalidTokenError("not an access token")
        return claims

    def verify_refresh(self, token: str) -> dict:
        claims = self._decode(token, self._refresh_secret)
        if claims.get("typ") != "refresh":
            raise jwt.InvalidTokenError("not a refresh token")
        return claims
