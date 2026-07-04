package catalog

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"

	"ecommerce/internal/db"
	"ecommerce/internal/pagination"
)

const cols = "id, sku, name, slug, description, price_cents, currency, stock_quantity, category_id, status, created_at, updated_at"

func scanProduct(row interface{ Scan(...any) error }) (Product, error) {
	var p Product
	err := row.Scan(&p.ID, &p.SKU, &p.Name, &p.Slug, &p.Description, &p.PriceCents,
		&p.Currency, &p.StockQuantity, &p.CategoryID, &p.Status, &p.CreatedAt, &p.UpdatedAt)
	return p, err
}

func ListProducts(ctx context.Context, q db.Querier, categoryID *uuid.UUID, search string, limit int, cursor *pagination.Cursor) ([]Product, error) {
	sql := "SELECT " + cols + " FROM products WHERE status = 'active'"
	args := []any{}
	i := 1

	if categoryID != nil {
		sql += fmt.Sprintf(" AND category_id = $%d", i)
		args = append(args, *categoryID)
		i++
	}
	if strings.TrimSpace(search) != "" {
		sql += fmt.Sprintf(" AND search_vector @@ websearch_to_tsquery('english', $%d)", i)
		args = append(args, search)
		i++
	}
	if cursor != nil {
		ts, err := time.Parse(time.RFC3339, cursor.TS)
		if err != nil {
			return nil, err
		}
		id, err := uuid.Parse(cursor.ID)
		if err != nil {
			return nil, err
		}
		sql += fmt.Sprintf(" AND (created_at, id) < ($%d, $%d)", i, i+1)
		args = append(args, ts, id)
		i += 2
	}
	sql += fmt.Sprintf(" ORDER BY created_at DESC, id DESC LIMIT $%d", i)
	args = append(args, limit+1)

	rows, err := q.Query(ctx, sql, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []Product
	for rows.Next() {
		p, err := scanProduct(rows)
		if err != nil {
			return nil, err
		}
		out = append(out, p)
	}
	return out, rows.Err()
}

func GetProduct(ctx context.Context, q db.Querier, id uuid.UUID) (Product, error) {
	return scanProduct(q.QueryRow(ctx, "SELECT "+cols+" FROM products WHERE id = $1", id))
}

func CreateProduct(ctx context.Context, q db.Querier, r CreateProductRequest) (Product, error) {
	currency := r.Currency
	if currency == "" {
		currency = "USD"
	}
	return scanProduct(q.QueryRow(ctx,
		"INSERT INTO products (sku, name, slug, description, price_cents, currency, stock_quantity, category_id) "+
			"VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING "+cols,
		r.SKU, r.Name, r.Slug, r.Description, r.PriceCents, currency, r.StockQuantity, r.CategoryID))
}

func UpdateProduct(ctx context.Context, q db.Querier, id uuid.UUID, r UpdateProductRequest) (Product, error) {
	sets := []string{"updated_at = now()"}
	args := []any{}
	i := 1
	add := func(col string, val any) {
		sets = append(sets, fmt.Sprintf("%s = $%d", col, i))
		args = append(args, val)
		i++
	}
	if r.Name != nil {
		add("name", *r.Name)
	}
	if r.Description != nil {
		add("description", *r.Description)
	}
	if r.PriceCents != nil {
		add("price_cents", *r.PriceCents)
	}
	if r.StockQuantity != nil {
		add("stock_quantity", *r.StockQuantity)
	}
	if r.CategoryID != nil {
		add("category_id", *r.CategoryID)
	}
	if r.Status != nil {
		add("status", *r.Status)
	}

	sql := fmt.Sprintf("UPDATE products SET %s WHERE id = $%d RETURNING %s",
		strings.Join(sets, ", "), i, cols)
	args = append(args, id)
	return scanProduct(q.QueryRow(ctx, sql, args...))
}

func ArchiveProduct(ctx context.Context, q db.Querier, id uuid.UUID) (bool, error) {
	tag, err := q.Exec(ctx, "UPDATE products SET status = 'archived', updated_at = now() WHERE id = $1", id)
	if err != nil {
		return false, err
	}
	return tag.RowsAffected() > 0, nil
}

func ListCategories(ctx context.Context, q db.Querier) ([]Category, error) {
	rows, err := q.Query(ctx, "SELECT id, name, slug, parent_id, created_at FROM categories ORDER BY name")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []Category
	for rows.Next() {
		var c Category
		if err := rows.Scan(&c.ID, &c.Name, &c.Slug, &c.ParentID, &c.CreatedAt); err != nil {
			return nil, err
		}
		out = append(out, c)
	}
	return out, rows.Err()
}

func CreateCategory(ctx context.Context, q db.Querier, r CreateCategoryRequest) (Category, error) {
	var c Category
	err := q.QueryRow(ctx,
		"INSERT INTO categories (name, slug, parent_id) VALUES ($1,$2,$3) RETURNING id, name, slug, parent_id, created_at",
		r.Name, r.Slug, r.ParentID).Scan(&c.ID, &c.Name, &c.Slug, &c.ParentID, &c.CreatedAt)
	return c, err
}
