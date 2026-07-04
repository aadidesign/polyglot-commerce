"""Opaque keyset (cursor) pagination helpers."""
from __future__ import annotations

import base64
import json

from app.errors import bad_request

DEFAULT = 20
MAX = 100


def clamp_limit(raw: str | None) -> int:
    try:
        n = int(raw) if raw else DEFAULT
    except (TypeError, ValueError):
        return DEFAULT
    if n <= 0:
        return DEFAULT
    return min(n, MAX)


def encode_cursor(ts: str, id_: str) -> str:
    raw = json.dumps({"ts": ts, "id": id_}).encode()
    return base64.urlsafe_b64encode(raw).decode().rstrip("=")


def decode_cursor(raw: str) -> dict:
    try:
        padded = raw + "=" * (-len(raw) % 4)
        return json.loads(base64.urlsafe_b64decode(padded))
    except Exception as exc:  # noqa: BLE001
        raise bad_request("invalid cursor") from exc


def page(items: list, next_cursor: str | None) -> dict:
    return {"items": items, "next_cursor": next_cursor, "has_more": next_cursor is not None}
