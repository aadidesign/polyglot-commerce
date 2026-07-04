package orders

import (
	"time"

	"github.com/google/uuid"
)

type Order struct {
	ID         uuid.UUID
	UserID     uuid.UUID
	Status     string
	TotalCents int64
	Currency   string
	CreatedAt  time.Time
}

type OrderItem struct {
	ProductID      uuid.UUID
	ProductName    string
	UnitPriceCents int64
	Quantity       int
}

type OrderItemResponse struct {
	ProductID      uuid.UUID `json:"product_id"`
	ProductName    string    `json:"product_name"`
	UnitPriceCents int64     `json:"unit_price_cents"`
	Quantity       int       `json:"quantity"`
	LineTotalCents int64     `json:"line_total_cents"`
}

type OrderResponse struct {
	ID         uuid.UUID           `json:"id"`
	Status     string              `json:"status"`
	TotalCents int64               `json:"total_cents"`
	Currency   string              `json:"currency"`
	CreatedAt  time.Time           `json:"created_at"`
	Items      []OrderItemResponse `json:"items"`
}

func NewOrderResponse(o Order, items []OrderItem) OrderResponse {
	resp := OrderResponse{
		ID:         o.ID,
		Status:     o.Status,
		TotalCents: o.TotalCents,
		Currency:   o.Currency,
		CreatedAt:  o.CreatedAt,
		Items:      []OrderItemResponse{},
	}
	for _, it := range items {
		resp.Items = append(resp.Items, OrderItemResponse{
			ProductID:      it.ProductID,
			ProductName:    it.ProductName,
			UnitPriceCents: it.UnitPriceCents,
			Quantity:       it.Quantity,
			LineTotalCents: it.UnitPriceCents * int64(it.Quantity),
		})
	}
	return resp
}

type OrderSummary struct {
	ID         uuid.UUID `json:"id"`
	Status     string    `json:"status"`
	TotalCents int64     `json:"total_cents"`
	Currency   string    `json:"currency"`
	CreatedAt  time.Time `json:"created_at"`
}
