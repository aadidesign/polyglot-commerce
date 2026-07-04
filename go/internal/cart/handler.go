package cart

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"

	"ecommerce/internal/auth"
	"ecommerce/internal/httpx"
)

type Handlers struct {
	Pool *pgxpool.Pool
}

func (h *Handlers) respondCart(w http.ResponseWriter, r *http.Request, userID uuid.UUID) {
	rows, err := GetCart(r.Context(), h.Pool, userID)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	httpx.JSON(w, http.StatusOK, BuildCart(rows))
}

func (h *Handlers) Get(w http.ResponseWriter, r *http.Request) {
	userID, err := auth.UserID(r.Context())
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	h.respondCart(w, r, userID)
}

func (h *Handlers) Add(w http.ResponseWriter, r *http.Request) {
	userID, err := auth.UserID(r.Context())
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	var req AddItemRequest
	if err := httpx.Decode(r, &req); err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := req.Validate(); err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := AddItem(r.Context(), h.Pool, userID, req.ProductID, req.Quantity); err != nil {
		httpx.Error(w, r, err)
		return
	}
	h.respondCart(w, r, userID)
}

func (h *Handlers) Update(w http.ResponseWriter, r *http.Request) {
	userID, err := auth.UserID(r.Context())
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	productID, err := uuid.Parse(chi.URLParam(r, "product_id"))
	if err != nil {
		httpx.Error(w, r, httpx.BadRequest("invalid product id"))
		return
	}
	var req UpdateItemRequest
	if err := httpx.Decode(r, &req); err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := req.Validate(); err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := SetItem(r.Context(), h.Pool, userID, productID, req.Quantity); err != nil {
		httpx.Error(w, r, err)
		return
	}
	h.respondCart(w, r, userID)
}

func (h *Handlers) Remove(w http.ResponseWriter, r *http.Request) {
	userID, err := auth.UserID(r.Context())
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	productID, err := uuid.Parse(chi.URLParam(r, "product_id"))
	if err != nil {
		httpx.Error(w, r, httpx.BadRequest("invalid product id"))
		return
	}
	if err := RemoveItem(r.Context(), h.Pool, userID, productID); err != nil {
		httpx.Error(w, r, err)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (h *Handlers) Clear(w http.ResponseWriter, r *http.Request) {
	userID, err := auth.UserID(r.Context())
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := Clear(r.Context(), h.Pool, userID); err != nil {
		httpx.Error(w, r, err)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}
