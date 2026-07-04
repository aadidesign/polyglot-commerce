// Package server wires the router: middleware stack, health/metrics, and all
// domain routes (public vs. authenticated).
package server

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"github.com/redis/go-redis/v9"

	"ecommerce/internal/auth"
	"ecommerce/internal/cart"
	"ecommerce/internal/catalog"
	"ecommerce/internal/config"
	"ecommerce/internal/httpx"
	"ecommerce/internal/orders"
)

func New(cfg config.Config, pool *pgxpool.Pool, rdb *redis.Client, mgr *auth.Manager) http.Handler {
	r := chi.NewRouter()
	r.Use(httpx.RequestID, httpx.CORS, httpx.Recover, httpx.Observe)

	// Operational endpoints.
	r.Get("/healthz", func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})
	r.Get("/readyz", func(w http.ResponseWriter, req *http.Request) {
		if err := pool.Ping(req.Context()); err != nil {
			httpx.Error(w, req, httpx.Internal())
			return
		}
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ready"))
	})
	r.Handle("/metrics", promhttp.Handler())

	ah := &auth.Handlers{Pool: pool, Mgr: mgr}
	ch := &catalog.Handlers{Pool: pool, Redis: rdb, CacheTTL: cfg.CacheTTL}
	carth := &cart.Handlers{Pool: pool}
	oh := &orders.Handlers{Pool: pool}

	// ---- Public routes ----
	r.Post("/v1/auth/register", ah.Register)
	r.Post("/v1/auth/login", ah.Login)
	r.Post("/v1/auth/refresh", ah.Refresh)
	r.Post("/v1/auth/logout", ah.Logout)
	r.Get("/v1/products", ch.List)
	r.Get("/v1/products/{id}", ch.Get)
	r.Get("/v1/categories", ch.ListCategories)

	// ---- Authenticated routes ----
	r.Group(func(pr chi.Router) {
		pr.Use(mgr.Authenticate)

		pr.Get("/v1/auth/me", ah.Me)

		// admin-gated inside handlers via RBAC (catalog:write)
		pr.Post("/v1/products", ch.Create)
		pr.Patch("/v1/products/{id}", ch.Update)
		pr.Delete("/v1/products/{id}", ch.Archive)
		pr.Post("/v1/categories", ch.CreateCategory)

		pr.Get("/v1/cart", carth.Get)
		pr.Delete("/v1/cart", carth.Clear)
		pr.Post("/v1/cart/items", carth.Add)
		pr.Patch("/v1/cart/items/{product_id}", carth.Update)
		pr.Delete("/v1/cart/items/{product_id}", carth.Remove)

		pr.Post("/v1/orders", oh.Checkout)
		pr.Get("/v1/orders", oh.List)
		pr.Get("/v1/orders/{id}", oh.Get)
		pr.Post("/v1/orders/{id}/cancel", oh.Cancel)
	})

	return r
}
