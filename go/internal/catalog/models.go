package catalog

import (
	"time"

	"github.com/google/uuid"

	"ecommerce/internal/httpx"
)

type Product struct {
	ID            uuid.UUID  `json:"id"`
	SKU           string     `json:"sku"`
	Name          string     `json:"name"`
	Slug          string     `json:"slug"`
	Description   string     `json:"description"`
	PriceCents    int64      `json:"price_cents"`
	Currency      string     `json:"currency"`
	StockQuantity int        `json:"stock_quantity"`
	CategoryID    *uuid.UUID `json:"category_id"`
	Status        string     `json:"status"`
	CreatedAt     time.Time  `json:"created_at"`
	UpdatedAt     time.Time  `json:"updated_at"`
}

type Category struct {
	ID        uuid.UUID  `json:"id"`
	Name      string     `json:"name"`
	Slug      string     `json:"slug"`
	ParentID  *uuid.UUID `json:"parent_id"`
	CreatedAt time.Time  `json:"created_at"`
}

type CreateProductRequest struct {
	SKU           string     `json:"sku"`
	Name          string     `json:"name"`
	Slug          string     `json:"slug"`
	Description   string     `json:"description"`
	PriceCents    int64      `json:"price_cents"`
	Currency      string     `json:"currency"`
	StockQuantity int        `json:"stock_quantity"`
	CategoryID    *uuid.UUID `json:"category_id"`
}

func (r CreateProductRequest) Validate() error {
	if r.SKU == "" || r.Name == "" || r.Slug == "" {
		return httpx.BadRequest("sku, name and slug are required")
	}
	if r.PriceCents < 0 || r.StockQuantity < 0 {
		return httpx.BadRequest("price_cents and stock_quantity must be non-negative")
	}
	return nil
}

type UpdateProductRequest struct {
	Name          *string    `json:"name"`
	Description   *string    `json:"description"`
	PriceCents    *int64     `json:"price_cents"`
	StockQuantity *int       `json:"stock_quantity"`
	CategoryID    *uuid.UUID `json:"category_id"`
	Status        *string    `json:"status"`
}

type CreateCategoryRequest struct {
	Name     string     `json:"name"`
	Slug     string     `json:"slug"`
	ParentID *uuid.UUID `json:"parent_id"`
}

func (r CreateCategoryRequest) Validate() error {
	if r.Name == "" || r.Slug == "" {
		return httpx.BadRequest("name and slug are required")
	}
	return nil
}
