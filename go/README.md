# E-Commerce Backend — Go

A production-grade e-commerce REST API built with **Go + chi + pgx**. One of
three independent implementations of the same contract (see the
[root README](../README.md) for the Rust and Python siblings and a comparison).

## Highlights

- **Auth** — registration, login, **Argon2id** hashing (`x/crypto/argon2`),
  JWT **access + refresh tokens with rotation & reuse detection**, **RBAC**.
- **Catalog** — product/category CRUD, **Postgres full-text search**, **keyset
  (cursor) pagination**, optional **Redis** read-through caching.
- **Cart** — durable per-user cart.
- **Orders** — checkout as a **single ACID transaction** with `SELECT … FOR
  UPDATE` row locking (no oversell), price snapshotting, restock-on-cancel.
- **Cross-cutting** — RFC 7807 `problem+json` errors, request-id propagation,
  `log/slog` structured logs, **Prometheus** `/metrics`, CORS, panic recovery,
  graceful shutdown, embedded SQL migrations, distroless Docker image.

## Layout

```
cmd/server/main.go        bootstrap: config → db → migrate → redis → serve
internal/
├── config/               typed env configuration
├── db/                   pgx pool, embedded-SQL migrator, Querier interface
├── httpx/                RFC 7807 errors, JSON helpers, middleware (id/log/metrics/cors/recover)
├── pagination/           generic keyset cursor + Page[T]
├── auth/                 argon2 · jwt · middleware · repo · handlers
├── catalog/              products/categories · search · cache · handlers
├── cart/                 per-user cart
├── orders/              checkout (ACID) · list · get · cancel
└── server/               router wiring (public vs authenticated)
migrations/*.sql          embedded via go:embed
```

The `db.Querier` interface is satisfied by both `*pgxpool.Pool` and `pgx.Tx`, so
repositories run unchanged inside or outside a transaction.

## Running

### Docker

```bash
docker compose up -d --build
curl http://localhost:8080/healthz
```

### Local

```bash
cp .env.example .env
docker compose up -d postgres redis
go run ./cmd/server
```

## API

Identical contract to the other implementations — see the
[OpenAPI spec](../api/openapi.yaml) and the endpoint table in the
[root README](../README.md). Base URL `http://localhost:8080`.

## Testing

```bash
go test ./...     # argon2 + JWT/RBAC unit tests
go vet ./...
```

## Tech stack

| Concern | Choice |
|---|---|
| Router | go-chi/chi v5 |
| DB | PostgreSQL via jackc/pgx v5 (pooled) |
| Cache | go-redis v9 (optional) |
| Auth | golang-jwt v5 + x/crypto/argon2 |
| Metrics | prometheus/client_golang |
| Logs | stdlib log/slog (JSON) |
| Money | integer cents (`int64`) |
