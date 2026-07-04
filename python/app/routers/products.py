"""Product & category endpoints: CRUD, full-text search, cursor pagination,
optional Redis read-through caching. Writes require the `catalog:write`
permission (RBAC)."""
from __future__ import annotations

import json
import uuid
from datetime import datetime

import asyncpg
from fastapi import APIRouter, Depends, Query

from app import pagination
from app.deps import get_pool, get_redis, get_settings, require_permission
from app.errors import bad_request, conflict, not_found
from app.schemas import CreateCategoryRequest, CreateProductRequest, UpdateProductRequest

router = APIRouter(prefix="/v1", tags=["catalog"])

COLS = (
    "id, sku, name, slug, description, price_cents, currency, "
    "stock_quantity, category_id, status, created_at, updated_at"
)


def _product(row) -> dict:
    return {
        "id": str(row["id"]),
        "sku": row["sku"],
        "name": row["name"],
        "slug": row["slug"],
        "description": row["description"],
        "price_cents": row["price_cents"],
        "currency": row["currency"],
        "stock_quantity": row["stock_quantity"],
        "category_id": str(row["category_id"]) if row["category_id"] else None,
        "status": row["status"],
        "created_at": row["created_at"].isoformat(),
        "updated_at": row["updated_at"].isoformat(),
    }


def _category(row) -> dict:
    return {
        "id": str(row["id"]),
        "name": row["name"],
        "slug": row["slug"],
        "parent_id": str(row["parent_id"]) if row["parent_id"] else None,
        "created_at": row["created_at"].isoformat(),
    }


@router.get("/products")
async def list_products(
    limit: int | None = Query(default=None),
    cursor: str | None = Query(default=None),
    category_id: str | None = Query(default=None),
    q: str | None = Query(default=None),
    pool=Depends(get_pool),
):
    lim = min(limit, pagination.MAX) if limit and limit > 0 else pagination.DEFAULT

    sql = f"SELECT {COLS} FROM products WHERE status = 'active'"
    args: list = []
    i = 1
    if category_id:
        try:
            args.append(uuid.UUID(category_id))
        except ValueError:
            raise bad_request("invalid category_id")
        sql += f" AND category_id = ${i}"
        i += 1
    if q and q.strip():
        sql += f" AND search_vector @@ websearch_to_tsquery('english', ${i})"
        args.append(q)
        i += 1
    if cursor:
        c = pagination.decode_cursor(cursor)
        try:
            ts = datetime.fromisoformat(c["ts"])
            cid = uuid.UUID(c["id"])
        except (KeyError, ValueError):
            raise bad_request("invalid cursor")
        sql += f" AND (created_at, id) < (${i}, ${i + 1})"
        args.extend([ts, cid])
        i += 2
    sql += f" ORDER BY created_at DESC, id DESC LIMIT ${i}"
    args.append(lim + 1)

    async with pool.acquire() as conn:
        rows = await conn.fetch(sql, *args)

    items = [_product(r) for r in rows]
    next_cursor = None
    if len(items) > lim:
        items = items[:lim]
        last = items[-1]
        next_cursor = pagination.encode_cursor(last["created_at"], last["id"])
    return pagination.page(items, next_cursor)


@router.get("/products/{product_id}")
async def get_product(
    product_id: uuid.UUID,
    pool=Depends(get_pool),
    redis=Depends(get_redis),
    settings=Depends(get_settings),
):
    key = f"catalog:product:{product_id}"
    if redis is not None:
        try:
            cached = await redis.get(key)
            if cached:
                return json.loads(cached)
        except Exception:  # noqa: BLE001
            pass

    async with pool.acquire() as conn:
        row = await conn.fetchrow(f"SELECT {COLS} FROM products WHERE id = $1", product_id)
    if row is None:
        raise not_found("product")
    product = _product(row)

    if redis is not None:
        try:
            await redis.set(key, json.dumps(product), ex=settings.cache_ttl_secs)
        except Exception:  # noqa: BLE001
            pass
    return product


@router.post("/products", status_code=201, dependencies=[Depends(require_permission("catalog:write"))])
async def create_product(body: CreateProductRequest, pool=Depends(get_pool)):
    async with pool.acquire() as conn:
        try:
            row = await conn.fetchrow(
                f"INSERT INTO products (sku, name, slug, description, price_cents, currency, "
                f"stock_quantity, category_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING {COLS}",
                body.sku, body.name, body.slug, body.description, body.price_cents,
                body.currency, body.stock_quantity, body.category_id,
            )
        except asyncpg.UniqueViolationError:
            raise conflict("sku or slug already exists")
    return _product(row)


@router.patch("/products/{product_id}", dependencies=[Depends(require_permission("catalog:write"))])
async def update_product(
    product_id: uuid.UUID,
    body: UpdateProductRequest,
    pool=Depends(get_pool),
    redis=Depends(get_redis),
):
    sets = ["updated_at = now()"]
    args: list = []
    i = 1
    for col, val in [
        ("name", body.name),
        ("description", body.description),
        ("price_cents", body.price_cents),
        ("stock_quantity", body.stock_quantity),
        ("category_id", body.category_id),
        ("status", body.status),
    ]:
        if val is not None:
            sets.append(f"{col} = ${i}")
            args.append(val)
            i += 1
    args.append(product_id)
    sql = f"UPDATE products SET {', '.join(sets)} WHERE id = ${i} RETURNING {COLS}"

    async with pool.acquire() as conn:
        row = await conn.fetchrow(sql, *args)
    if row is None:
        raise not_found("product")
    if redis is not None:
        try:
            await redis.delete(f"catalog:product:{product_id}")
        except Exception:  # noqa: BLE001
            pass
    return _product(row)


@router.delete("/products/{product_id}", status_code=204,
               dependencies=[Depends(require_permission("catalog:write"))])
async def archive_product(product_id: uuid.UUID, pool=Depends(get_pool), redis=Depends(get_redis)):
    async with pool.acquire() as conn:
        result = await conn.execute(
            "UPDATE products SET status = 'archived', updated_at = now() WHERE id = $1", product_id
        )
    # asyncpg returns a command tag like "UPDATE 1"; the last token is the count.
    if result.split()[-1] == "0":
        raise not_found("product")
    if redis is not None:
        try:
            await redis.delete(f"catalog:product:{product_id}")
        except Exception:  # noqa: BLE001
            pass
    return None


@router.get("/categories")
async def list_categories(pool=Depends(get_pool)):
    async with pool.acquire() as conn:
        rows = await conn.fetch(
            "SELECT id, name, slug, parent_id, created_at FROM categories ORDER BY name"
        )
    return [_category(r) for r in rows]


@router.post("/categories", status_code=201,
             dependencies=[Depends(require_permission("catalog:write"))])
async def create_category(body: CreateCategoryRequest, pool=Depends(get_pool)):
    async with pool.acquire() as conn:
        try:
            row = await conn.fetchrow(
                "INSERT INTO categories (name, slug, parent_id) VALUES ($1,$2,$3) "
                "RETURNING id, name, slug, parent_id, created_at",
                body.name, body.slug, body.parent_id,
            )
        except asyncpg.UniqueViolationError:
            raise conflict("slug already exists")
    return _category(row)
