package orders

import (
	"errors"
	"net/http"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"

	"ecommerce/internal/auth"
	"ecommerce/internal/httpx"
	"ecommerce/internal/pagination"
)

type Handlers struct {
	Pool *pgxpool.Pool
}

func (h *Handlers) Checkout(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	if err := auth.Require(ctx, "order:write"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	userID, _ := auth.UserID(ctx)

	order, err := Checkout(ctx, h.Pool, userID)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	items, err := OrderItems(ctx, h.Pool, order.ID)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	httpx.JSON(w, http.StatusCreated, NewOrderResponse(order, items))
}

func (h *Handlers) List(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	if err := auth.Require(ctx, "order:read"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	userID, _ := auth.UserID(ctx)

	limit := pagination.ClampLimit(r.URL.Query().Get("limit"))
	var cursor *pagination.Cursor
	if cs := r.URL.Query().Get("cursor"); cs != "" {
		c, err := pagination.Decode(cs)
		if err != nil {
			httpx.Error(w, r, httpx.BadRequest("invalid cursor"))
			return
		}
		cursor = &c
	}

	list, err := ListOrders(ctx, h.Pool, userID, limit, cursor)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}

	var next *string
	if len(list) > limit {
		list = list[:limit]
		last := list[limit-1]
		c := pagination.Cursor{TS: last.CreatedAt.Format(time.RFC3339), ID: last.ID.String()}.Encode()
		next = &c
	}

	summaries := make([]OrderSummary, 0, len(list))
	for _, o := range list {
		summaries = append(summaries, OrderSummary{
			ID: o.ID, Status: o.Status, TotalCents: o.TotalCents,
			Currency: o.Currency, CreatedAt: o.CreatedAt,
		})
	}
	httpx.JSON(w, http.StatusOK, pagination.NewPage(summaries, next))
}

func (h *Handlers) Get(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	if err := auth.Require(ctx, "order:read"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	userID, _ := auth.UserID(ctx)
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		httpx.Error(w, r, httpx.BadRequest("invalid order id"))
		return
	}

	order, err := GetOrder(ctx, h.Pool, userID, id)
	if errors.Is(err, pgx.ErrNoRows) {
		httpx.Error(w, r, httpx.NotFound("order"))
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	items, err := OrderItems(ctx, h.Pool, order.ID)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	httpx.JSON(w, http.StatusOK, NewOrderResponse(order, items))
}

func (h *Handlers) Cancel(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	if err := auth.Require(ctx, "order:write"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	userID, _ := auth.UserID(ctx)
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		httpx.Error(w, r, httpx.BadRequest("invalid order id"))
		return
	}

	order, err := Cancel(ctx, h.Pool, userID, id)
	if errors.Is(err, pgx.ErrNoRows) {
		httpx.Error(w, r, httpx.NotFound("order"))
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	items, err := OrderItems(ctx, h.Pool, order.ID)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	httpx.JSON(w, http.StatusOK, NewOrderResponse(order, items))
}
