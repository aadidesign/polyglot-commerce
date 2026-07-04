package catalog

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgconn"
	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/redis/go-redis/v9"

	"ecommerce/internal/auth"
	"ecommerce/internal/httpx"
	"ecommerce/internal/pagination"
)

type Handlers struct {
	Pool     *pgxpool.Pool
	Redis    *redis.Client // may be nil
	CacheTTL time.Duration
}

func isUniqueViolation(err error) bool {
	var pgErr *pgconn.PgError
	return errors.As(err, &pgErr) && pgErr.Code == "23505"
}

func (h *Handlers) cacheKey(id uuid.UUID) string { return "catalog:product:" + id.String() }

func (h *Handlers) cacheGet(ctx context.Context, id uuid.UUID) (Product, bool) {
	if h.Redis == nil {
		return Product{}, false
	}
	val, err := h.Redis.Get(ctx, h.cacheKey(id)).Result()
	if err != nil {
		return Product{}, false
	}
	var p Product
	if json.Unmarshal([]byte(val), &p) != nil {
		return Product{}, false
	}
	return p, true
}

func (h *Handlers) cachePut(ctx context.Context, p Product) {
	if h.Redis == nil {
		return
	}
	if b, err := json.Marshal(p); err == nil {
		h.Redis.Set(ctx, h.cacheKey(p.ID), b, h.CacheTTL)
	}
}

func (h *Handlers) cacheDel(ctx context.Context, id uuid.UUID) {
	if h.Redis != nil {
		h.Redis.Del(ctx, h.cacheKey(id))
	}
}

func (h *Handlers) List(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	qp := r.URL.Query()
	limit := pagination.ClampLimit(qp.Get("limit"))

	var cursor *pagination.Cursor
	if cs := qp.Get("cursor"); cs != "" {
		c, err := pagination.Decode(cs)
		if err != nil {
			httpx.Error(w, r, httpx.BadRequest("invalid cursor"))
			return
		}
		cursor = &c
	}
	var catID *uuid.UUID
	if cid := qp.Get("category_id"); cid != "" {
		id, err := uuid.Parse(cid)
		if err != nil {
			httpx.Error(w, r, httpx.BadRequest("invalid category_id"))
			return
		}
		catID = &id
	}

	products, err := ListProducts(ctx, h.Pool, catID, qp.Get("q"), limit, cursor)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}

	var next *string
	if len(products) > limit {
		products = products[:limit]
		last := products[limit-1]
		c := pagination.Cursor{TS: last.CreatedAt.Format(time.RFC3339), ID: last.ID.String()}.Encode()
		next = &c
	}
	if products == nil {
		products = []Product{}
	}
	httpx.JSON(w, http.StatusOK, pagination.NewPage(products, next))
}

func (h *Handlers) Get(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		httpx.Error(w, r, httpx.BadRequest("invalid product id"))
		return
	}
	if p, ok := h.cacheGet(ctx, id); ok {
		httpx.JSON(w, http.StatusOK, p)
		return
	}
	p, err := GetProduct(ctx, h.Pool, id)
	if errors.Is(err, pgx.ErrNoRows) {
		httpx.Error(w, r, httpx.NotFound("product"))
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	h.cachePut(ctx, p)
	httpx.JSON(w, http.StatusOK, p)
}

func (h *Handlers) Create(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	if err := auth.Require(ctx, "catalog:write"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	var req CreateProductRequest
	if err := httpx.Decode(r, &req); err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := req.Validate(); err != nil {
		httpx.Error(w, r, err)
		return
	}
	p, err := CreateProduct(ctx, h.Pool, req)
	if isUniqueViolation(err) {
		httpx.Error(w, r, httpx.Conflict("sku or slug already exists"))
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	httpx.JSON(w, http.StatusCreated, p)
}

func (h *Handlers) Update(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	if err := auth.Require(ctx, "catalog:write"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		httpx.Error(w, r, httpx.BadRequest("invalid product id"))
		return
	}
	var req UpdateProductRequest
	if err := httpx.Decode(r, &req); err != nil {
		httpx.Error(w, r, err)
		return
	}
	p, err := UpdateProduct(ctx, h.Pool, id, req)
	if errors.Is(err, pgx.ErrNoRows) {
		httpx.Error(w, r, httpx.NotFound("product"))
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	h.cacheDel(ctx, id)
	httpx.JSON(w, http.StatusOK, p)
}

func (h *Handlers) Archive(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	if err := auth.Require(ctx, "catalog:write"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		httpx.Error(w, r, httpx.BadRequest("invalid product id"))
		return
	}
	ok, err := ArchiveProduct(ctx, h.Pool, id)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	if !ok {
		httpx.Error(w, r, httpx.NotFound("product"))
		return
	}
	h.cacheDel(ctx, id)
	w.WriteHeader(http.StatusNoContent)
}

func (h *Handlers) ListCategories(w http.ResponseWriter, r *http.Request) {
	cats, err := ListCategories(r.Context(), h.Pool)
	if err != nil {
		httpx.Error(w, r, err)
		return
	}
	if cats == nil {
		cats = []Category{}
	}
	httpx.JSON(w, http.StatusOK, cats)
}

func (h *Handlers) CreateCategory(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	if err := auth.Require(ctx, "catalog:write"); err != nil {
		httpx.Error(w, r, err)
		return
	}
	var req CreateCategoryRequest
	if err := httpx.Decode(r, &req); err != nil {
		httpx.Error(w, r, err)
		return
	}
	if err := req.Validate(); err != nil {
		httpx.Error(w, r, err)
		return
	}
	c, err := CreateCategory(ctx, h.Pool, req)
	if isUniqueViolation(err) {
		httpx.Error(w, r, httpx.Conflict("slug already exists"))
		return
	} else if err != nil {
		httpx.Error(w, r, err)
		return
	}
	httpx.JSON(w, http.StatusCreated, c)
}
