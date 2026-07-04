package cart

import (
	"github.com/google/uuid"

	"ecommerce/internal/httpx"
)

// CartRow is a cart line joined with its product (internal scan target).
type CartRow struct {
	ProductID  uuid.UUID
	Name       string
	PriceCents int64
	Currency   string
	Quantity   int
}

type CartItem struct {
	ProductID      uuid.UUID `json:"product_id"`
	Name           string    `json:"name"`
	UnitPriceCents int64     `json:"unit_price_cents"`
	Quantity       int       `json:"quantity"`
	LineTotalCents int64     `json:"line_total_cents"`
}

type CartResponse struct {
	Items      []CartItem `json:"items"`
	TotalCents int64      `json:"total_cents"`
	Currency   string     `json:"currency"`
}

func BuildCart(rows []CartRow) CartResponse {
	resp := CartResponse{Items: []CartItem{}, Currency: "USD"}
	for _, r := range rows {
		line := r.PriceCents * int64(r.Quantity)
		resp.Items = append(resp.Items, CartItem{
			ProductID:      r.ProductID,
			Name:           r.Name,
			UnitPriceCents: r.PriceCents,
			Quantity:       r.Quantity,
			LineTotalCents: line,
		})
		resp.TotalCents += line
		resp.Currency = r.Currency
	}
	return resp
}

type AddItemRequest struct {
	ProductID uuid.UUID `json:"product_id"`
	Quantity  int       `json:"quantity"`
}

func (r AddItemRequest) Validate() error {
	if r.Quantity < 1 || r.Quantity > 1000 {
		return httpx.BadRequest("quantity must be between 1 and 1000")
	}
	return nil
}

type UpdateItemRequest struct {
	Quantity int `json:"quantity"`
}

func (r UpdateItemRequest) Validate() error {
	if r.Quantity < 1 || r.Quantity > 1000 {
		return httpx.BadRequest("quantity must be between 1 and 1000")
	}
	return nil
}
