import uuid

import pytest

from app.config import Settings
from app.security import JWTManager, hash_password, verify_password


def _settings() -> Settings:
    return Settings(
        database_url="postgresql://localhost/test",
        jwt_access_secret="a" * 32,
        jwt_refresh_secret="b" * 32,
    )


def test_password_round_trip():
    h = hash_password("hunter2hunter2")
    assert verify_password("hunter2hunter2", h)
    assert not verify_password("wrong-password", h)


def test_access_token_round_trip_and_rbac():
    mgr = JWTManager(_settings())
    uid = uuid.uuid4()
    issued = mgr.issue_access(uid, ["customer"], ["order:write"])
    claims = mgr.verify_access(issued.token)
    assert claims["sub"] == str(uid)
    assert "order:write" in claims["permissions"]
    assert "catalog:write" not in claims["permissions"]


def test_access_verifier_rejects_refresh_token():
    mgr = JWTManager(_settings())
    issued = mgr.issue_refresh(uuid.uuid4(), uuid.uuid4())
    with pytest.raises(Exception):
        mgr.verify_access(issued.token)
