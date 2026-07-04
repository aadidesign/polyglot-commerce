package orders

import (
	"context"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"

	"ecommerce/internal/db"
	"ecommerce/internal/httpx"
	"ecommerce/internal/pagination"
)

const orderCols = "id, user_id, status, total_cents, currency, created_at"

func scanOrder(row interface{ Scan(...any) error }) (Order, error) {
	var o Order
	err := row.Scan(&o.ID, &o.UserID, &o.Status, &o.TotalCents, &o.Currency, &o.CreatedAt)
	return o, err
}

type checkoutRow struct {
	productID  uuid.UUID
	name       string
	priceCents int64
	currency   string
	stock      int
	quantity   int
}

// Checkout turns the user's cart into an order in a single transaction. Product
// rows are locked (FOR UPDATE) to prevent oversell; stock is decremented and the
// cart cleared atomically. Mock payment is assumed to succeed → 'confirmed'.
func Checkout(ctx context.Context, pool *pgxpool.Pool, userID uuid.UUID) (Order, error) {
	tx, err := pool.Begin(ctx)
	if err != nil {
		return Order{}, err
	}
	defer tx.Rollback(ctx)

	rows, err := tx.Query(ctx,
		`SELECT ci.product_id, p.name, p.price_cents, p.currency, p.stock_quantity, ci.quantity
		 FROM cart_items ci JOIN products p ON p.id = ci.product_id
		 WHERE ci.user_id = $1 FOR UPDATE OF p`, userID)
	if err != nil {
		return Order{}, err
	}
	var cart []checkoutRow
	for rows.Next() {
		var c checkoutRow
		if err := rows.Scan(&c.productID, &c.name, &c.priceCents, &c.currency, &c.stock, &c.quantity); err != nil {
			rows.Close()
			return Order{}, err
		}
		cart = append(cart, c)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return Order{}, err
	}
	if len(cart) == 0 {
		return Order{}, httpx.BadRequest("cart is empty")
	}

	var total int64
	for _, c := range cart {
		if c.quantity > c.stock {
			return Order{}, httpx.Conflict(fmt.Sprintf(
				"insufficient stock for '%s' (requested %d, available %d)", c.name, c.quantity, c.stock))
		}
		total += c.priceCents * int64(c.quantity)
	}

	order, err := scanOrder(tx.QueryRow(ctx,
		"INSERT INTO orders (user_id, status, total_cents, currency) VALUES ($1,'confirmed',$2,$3) RETURNING "+orderCols,
		userID, total, cart[0].currency))
	if err != nil {
		return Order{}, err
	}

	for _, c := range cart {
		if _, err := tx.Exec(ctx,
			"INSERT INTO order_items (order_id, product_id, product_name, unit_price_cents, quantity) VALUES ($1,$2,$3,$4,$5)",
			order.ID, c.productID, c.name, c.priceCents, c.quantity); err != nil {
			return Order{}, err
		}
		if _, err := tx.Exec(ctx,
			"UPDATE products SET stock_quantity = stock_quantity - $2 WHERE id = $1",
			c.productID, c.quantity); err != nil {
			return Order{}, err
		}
	}

	if _, err := tx.Exec(ctx, "DELETE FROM cart_items WHERE user_id = $1", userID); err != nil {
		return Order{}, err
	}
	if err := tx.Commit(ctx); err != nil {
		return Order{}, err
	}
	return order, nil
}

func ListOrders(ctx context.Context, q db.Querier, userID uuid.UUID, limit int, cursor *pagination.Cursor) ([]Order, error) {
	var (
		sql  string
		args []any
	)
	if cursor != nil {
		ts, err := time.Parse(time.RFC3339, cursor.TS)
		if err != nil {
			return nil, err
		}
		id, err := uuid.Parse(cursor.ID)
		if err != nil {
			return nil, err
		}
		sql = "SELECT " + orderCols + " FROM orders WHERE user_id = $1 AND (created_at, id) < ($2, $3) ORDER BY created_at DESC, id DESC LIMIT $4"
		args = []any{userID, ts, id, limit + 1}
	} else {
		sql = "SELECT " + orderCols + " FROM orders WHERE user_id = $1 ORDER BY created_at DESC, id DESC LIMIT $2"
		args = []any{userID, limit + 1}
	}

	rows, err := q.Query(ctx, sql, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Order
	for rows.Next() {
		o, err := scanOrder(rows)
		if err != nil {
			return nil, err
		}
		out = append(out, o)
	}
	return out, rows.Err()
}

func GetOrder(ctx context.Context, q db.Querier, userID, orderID uuid.UUID) (Order, error) {
	return scanOrder(q.QueryRow(ctx,
		"SELECT "+orderCols+" FROM orders WHERE id = $1 AND user_id = $2", orderID, userID))
}

func OrderItems(ctx context.Context, q db.Querier, orderID uuid.UUID) ([]OrderItem, error) {
	rows, err := q.Query(ctx,
		"SELECT product_id, product_name, unit_price_cents, quantity FROM order_items WHERE order_id = $1", orderID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []OrderItem
	for rows.Next() {
		var it OrderItem
		if err := rows.Scan(&it.ProductID, &it.ProductName, &it.UnitPriceCents, &it.Quantity); err != nil {
			return nil, err
		}
		out = append(out, it)
	}
	return out, rows.Err()
}

// Cancel cancels an order and restocks its items, atomically.
func Cancel(ctx context.Context, pool *pgxpool.Pool, userID, orderID uuid.UUID) (Order, error) {
	tx, err := pool.Begin(ctx)
	if err != nil {
		return Order{}, err
	}
	defer tx.Rollback(ctx)

	order, err := scanOrder(tx.QueryRow(ctx,
		"SELECT "+orderCols+" FROM orders WHERE id = $1 AND user_id = $2 FOR UPDATE", orderID, userID))
	if err != nil {
		return Order{}, err // pgx.ErrNoRows handled by caller
	}
	if order.Status == "cancelled" {
		return Order{}, httpx.Conflict("order is already cancelled")
	}

	items, err := OrderItems(ctx, tx, orderID)
	if err != nil {
		return Order{}, err
	}
	for _, it := range items {
		if _, err := tx.Exec(ctx,
			"UPDATE products SET stock_quantity = stock_quantity + $2 WHERE id = $1",
			it.ProductID, it.Quantity); err != nil {
			return Order{}, err
		}
	}

	updated, err := scanOrder(tx.QueryRow(ctx,
		"UPDATE orders SET status = 'cancelled', updated_at = now() WHERE id = $1 RETURNING "+orderCols, orderID))
	if err != nil {
		return Order{}, err
	}
	if err := tx.Commit(ctx); err != nil {
		return Order{}, err
	}
	return updated, nil
}
