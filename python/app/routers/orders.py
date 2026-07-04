"""Order endpoints. Checkout is a single ACID transaction (FOR UPDATE row
locking, no oversell); cancellation restocks. All authenticated + RBAC-gated."""
from __future__ import annotations

import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, Query

from app import pagination
from app.deps import current_user_id, get_pool, require_permission
from app.errors import bad_request, conflict, not_found

router = APIRouter(prefix="/v1/orders", tags=["orders"])


def _order(order_row, item_rows) -> dict:
    items = []
    for it in item_rows:
        items.append({
            "product_id": str(it["product_id"]),
            "product_name": it["product_name"],
            "unit_price_cents": it["unit_price_cents"],
            "quantity": it["quantity"],
            "line_total_cents": it["unit_price_cents"] * it["quantity"],
        })
    return {
        "id": str(order_row["id"]),
        "status": order_row["status"],
        "total_cents": order_row["total_cents"],
        "currency": order_row["currency"],
        "created_at": order_row["created_at"].isoformat(),
        "items": items,
    }


@router.post("", status_code=201, dependencies=[Depends(require_permission("order:write"))])
async def checkout(user_id: uuid.UUID = Depends(current_user_id), pool=Depends(get_pool)):
    async with pool.acquire() as conn:
        async with conn.transaction():
            cart = await conn.fetch(
                "SELECT ci.product_id, p.name, p.price_cents, p.currency, p.stock_quantity, ci.quantity "
                "FROM cart_items ci JOIN products p ON p.id = ci.product_id "
                "WHERE ci.user_id = $1 FOR UPDATE OF p",
                user_id,
            )
            if not cart:
                raise bad_request("cart is empty")

            total = 0
            for c in cart:
                if c["quantity"] > c["stock_quantity"]:
                    raise conflict(
                        f"insufficient stock for '{c['name']}' "
                        f"(requested {c['quantity']}, available {c['stock_quantity']})"
                    )
                total += c["price_cents"] * c["quantity"]

            order = await conn.fetchrow(
                "INSERT INTO orders (user_id, status, total_cents, currency) "
                "VALUES ($1,'confirmed',$2,$3) RETURNING id, status, total_cents, currency, created_at",
                user_id, total, cart[0]["currency"],
            )
            for c in cart:
                await conn.execute(
                    "INSERT INTO order_items (order_id, product_id, product_name, unit_price_cents, quantity) "
                    "VALUES ($1,$2,$3,$4,$5)",
                    order["id"], c["product_id"], c["name"], c["price_cents"], c["quantity"],
                )
                await conn.execute(
                    "UPDATE products SET stock_quantity = stock_quantity - $2 WHERE id = $1",
                    c["product_id"], c["quantity"],
                )
            await conn.execute("DELETE FROM cart_items WHERE user_id = $1", user_id)
            items = await conn.fetch(
                "SELECT product_id, product_name, unit_price_cents, quantity "
                "FROM order_items WHERE order_id = $1",
                order["id"],
            )
    return _order(order, items)


@router.get("", dependencies=[Depends(require_permission("order:read"))])
async def list_orders(
    limit: int | None = Query(default=None),
    cursor: str | None = Query(default=None),
    user_id: uuid.UUID = Depends(current_user_id),
    pool=Depends(get_pool),
):
    lim = min(limit, pagination.MAX) if limit and limit > 0 else pagination.DEFAULT
    cols = "id, status, total_cents, currency, created_at"

    if cursor:
        c = pagination.decode_cursor(cursor)
        try:
            ts = datetime.fromisoformat(c["ts"])
            cid = uuid.UUID(c["id"])
        except (KeyError, ValueError):
            raise bad_request("invalid cursor")
        sql = (f"SELECT {cols} FROM orders WHERE user_id = $1 AND (created_at, id) < ($2, $3) "
               f"ORDER BY created_at DESC, id DESC LIMIT $4")
        args = [user_id, ts, cid, lim + 1]
    else:
        sql = (f"SELECT {cols} FROM orders WHERE user_id = $1 "
               f"ORDER BY created_at DESC, id DESC LIMIT $2")
        args = [user_id, lim + 1]

    async with pool.acquire() as conn:
        rows = await conn.fetch(sql, *args)

    summaries = [{
        "id": str(r["id"]),
        "status": r["status"],
        "total_cents": r["total_cents"],
        "currency": r["currency"],
        "created_at": r["created_at"].isoformat(),
    } for r in rows]

    next_cursor = None
    if len(summaries) > lim:
        summaries = summaries[:lim]
        last = summaries[-1]
        next_cursor = pagination.encode_cursor(last["created_at"], last["id"])
    return pagination.page(summaries, next_cursor)


@router.get("/{order_id}", dependencies=[Depends(require_permission("order:read"))])
async def get_order(
    order_id: uuid.UUID,
    user_id: uuid.UUID = Depends(current_user_id),
    pool=Depends(get_pool),
):
    async with pool.acquire() as conn:
        order = await conn.fetchrow(
            "SELECT id, status, total_cents, currency, created_at FROM orders "
            "WHERE id = $1 AND user_id = $2",
            order_id, user_id,
        )
        if order is None:
            raise not_found("order")
        items = await conn.fetch(
            "SELECT product_id, product_name, unit_price_cents, quantity "
            "FROM order_items WHERE order_id = $1",
            order_id,
        )
    return _order(order, items)


@router.post("/{order_id}/cancel", dependencies=[Depends(require_permission("order:write"))])
async def cancel_order(
    order_id: uuid.UUID,
    user_id: uuid.UUID = Depends(current_user_id),
    pool=Depends(get_pool),
):
    async with pool.acquire() as conn:
        async with conn.transaction():
            order = await conn.fetchrow(
                "SELECT id, status, total_cents, currency, created_at FROM orders "
                "WHERE id = $1 AND user_id = $2 FOR UPDATE",
                order_id, user_id,
            )
            if order is None:
                raise not_found("order")
            if order["status"] == "cancelled":
                raise conflict("order is already cancelled")

            items = await conn.fetch(
                "SELECT product_id, product_name, unit_price_cents, quantity "
                "FROM order_items WHERE order_id = $1",
                order_id,
            )
            for it in items:
                await conn.execute(
                    "UPDATE products SET stock_quantity = stock_quantity + $2 WHERE id = $1",
                    it["product_id"], it["quantity"],
                )
            updated = await conn.fetchrow(
                "UPDATE orders SET status = 'cancelled', updated_at = now() WHERE id = $1 "
                "RETURNING id, status, total_cents, currency, created_at",
                order_id,
            )
    return _order(updated, items)
