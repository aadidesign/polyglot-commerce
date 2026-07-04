package cart

import (
	"context"

	"github.com/google/uuid"

	"ecommerce/internal/db"
	"ecommerce/internal/httpx"
)

func GetCart(ctx context.Context, q db.Querier, userID uuid.UUID) ([]CartRow, error) {
	rows, err := q.Query(ctx,
		`SELECT ci.product_id, p.name, p.price_cents, p.currency, ci.quantity
		 FROM cart_items ci JOIN products p ON p.id = ci.product_id
		 WHERE ci.user_id = $1 ORDER BY ci.added_at`, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []CartRow
	for rows.Next() {
		var c CartRow
		if err := rows.Scan(&c.ProductID, &c.Name, &c.PriceCents, &c.Currency, &c.Quantity); err != nil {
			return nil, err
		}
		out = append(out, c)
	}
	return out, rows.Err()
}

// AddItem creates or increments a line. 404 if the product is missing/archived.
func AddItem(ctx context.Context, q db.Querier, userID, productID uuid.UUID, qty int) error {
	var exists bool
	if err := q.QueryRow(ctx,
		"SELECT EXISTS(SELECT 1 FROM products WHERE id = $1 AND status = 'active')", productID,
	).Scan(&exists); err != nil {
		return err
	}
	if !exists {
		return httpx.NotFound("product")
	}
	_, err := q.Exec(ctx,
		`INSERT INTO cart_items (user_id, product_id, quantity) VALUES ($1, $2, $3)
		 ON CONFLICT (user_id, product_id) DO UPDATE
		 SET quantity = cart_items.quantity + EXCLUDED.quantity`,
		userID, productID, qty)
	return err
}

// SetItem sets an existing line to an absolute quantity. 404 if not present.
func SetItem(ctx context.Context, q db.Querier, userID, productID uuid.UUID, qty int) error {
	tag, err := q.Exec(ctx,
		"UPDATE cart_items SET quantity = $3 WHERE user_id = $1 AND product_id = $2",
		userID, productID, qty)
	if err != nil {
		return err
	}
	if tag.RowsAffected() == 0 {
		return httpx.NotFound("cart item")
	}
	return nil
}

func RemoveItem(ctx context.Context, q db.Querier, userID, productID uuid.UUID) error {
	_, err := q.Exec(ctx, "DELETE FROM cart_items WHERE user_id = $1 AND product_id = $2", userID, productID)
	return err
}

func Clear(ctx context.Context, q db.Querier, userID uuid.UUID) error {
	_, err := q.Exec(ctx, "DELETE FROM cart_items WHERE user_id = $1", userID)
	return err
}
