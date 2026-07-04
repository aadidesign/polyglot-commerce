"""Cart endpoints (all authenticated; the cart is owned by the token's user)."""
from __future__ import annotations

import uuid

from fastapi import APIRouter, Depends

from app.deps import current_user_id, get_pool
from app.errors import not_found
from app.schemas import AddItemRequest, UpdateItemRequest

router = APIRouter(prefix="/v1/cart", tags=["cart"])


async def _cart(conn, user_id: uuid.UUID) -> dict:
    rows = await conn.fetch(
        "SELECT ci.product_id, p.name, p.price_cents, p.currency, ci.quantity "
        "FROM cart_items ci JOIN products p ON p.id = ci.product_id "
        "WHERE ci.user_id = $1 ORDER BY ci.added_at",
        user_id,
    )
    items, total, currency = [], 0, "USD"
    for r in rows:
        line = r["price_cents"] * r["quantity"]
        total += line
        currency = r["currency"]
        items.append({
            "product_id": str(r["product_id"]),
            "name": r["name"],
            "unit_price_cents": r["price_cents"],
            "quantity": r["quantity"],
            "line_total_cents": line,
        })
    return {"items": items, "total_cents": total, "currency": currency}


@router.get("")
async def get_cart(user_id: uuid.UUID = Depends(current_user_id), pool=Depends(get_pool)):
    async with pool.acquire() as conn:
        return await _cart(conn, user_id)


@router.post("/items")
async def add_item(
    body: AddItemRequest,
    user_id: uuid.UUID = Depends(current_user_id),
    pool=Depends(get_pool),
):
    async with pool.acquire() as conn:
        exists = await conn.fetchval(
            "SELECT EXISTS(SELECT 1 FROM products WHERE id = $1 AND status = 'active')",
            body.product_id,
        )
        if not exists:
            raise not_found("product")
        await conn.execute(
            "INSERT INTO cart_items (user_id, product_id, quantity) VALUES ($1,$2,$3) "
            "ON CONFLICT (user_id, product_id) DO UPDATE "
            "SET quantity = cart_items.quantity + EXCLUDED.quantity",
            user_id, body.product_id, body.quantity,
        )
        return await _cart(conn, user_id)


@router.patch("/items/{product_id}")
async def update_item(
    product_id: uuid.UUID,
    body: UpdateItemRequest,
    user_id: uuid.UUID = Depends(current_user_id),
    pool=Depends(get_pool),
):
    async with pool.acquire() as conn:
        result = await conn.execute(
            "UPDATE cart_items SET quantity = $3 WHERE user_id = $1 AND product_id = $2",
            user_id, product_id, body.quantity,
        )
        if result.split()[-1] == "0":
            raise not_found("cart item")
        return await _cart(conn, user_id)


@router.delete("/items/{product_id}", status_code=204)
async def remove_item(
    product_id: uuid.UUID,
    user_id: uuid.UUID = Depends(current_user_id),
    pool=Depends(get_pool),
):
    async with pool.acquire() as conn:
        await conn.execute(
            "DELETE FROM cart_items WHERE user_id = $1 AND product_id = $2", user_id, product_id
        )
    return None


@router.delete("", status_code=204)
async def clear_cart(user_id: uuid.UUID = Depends(current_user_id), pool=Depends(get_pool)):
    async with pool.acquire() as conn:
        await conn.execute("DELETE FROM cart_items WHERE user_id = $1", user_id)
    return None
