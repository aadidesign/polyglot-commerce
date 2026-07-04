# E-Commerce Backend - Rust

A production-grade e-commerce REST API built with **Rust + axum + sqlx**. One of
three independent implementations of the same contract (see the
[root README](../README.md) for the Go and Python siblings and a feature
comparison).

## Highlights

- **Auth** - registration, login, **Argon2id** password hashing, JWT **access +
  refresh tokens with rotation & reuse detection**, **RBAC** (roles → permissions).
- **Catalog** - product/category CRUD, **Postgres full-text search**, **keyset
  (cursor) pagination**, **Redis** read-through caching with event-style invalidation.
- **Cart** - durable per-user cart, add/update/remove/clear.
- **Orders** - checkout as a **single ACID transaction** with `SELECT … FOR UPDATE`
  row locking (no oversell), price snapshotting, and **stock-restoring cancellation**.
- **Cross-cutting** - RFC 7807 `problem+json` errors, request-id propagation,
  Prometheus `/metrics`, structured JSON logs, CORS, gzip, timeouts, panic
  recovery, graceful shutdown, embedded SQL migrations, and a multi-stage
  cargo-chef Docker build.

## Architecture - modular monolith

Clear bounded modules over one PostgreSQL database. Because everything shares a
transaction boundary, checkout needs no distributed saga - it is one atomic
transaction, which is both simpler and stronger than eventual consistency.

```
src/
├── main.rs              bootstrap: config → db → migrate → redis → serve
├── config.rs            typed env configuration (12-Factor)
├── state.rs             shared AppState (pool, jwt, redis, metrics)
├── router.rs            route table + health/metrics + middleware stack
├── auth/                register · login · refresh-rotation · logout · me
├── products/            CRUD · search · cursor pagination · cache
├── cart/                per-user cart lines
└── orders/             checkout (ACID) · list · get · cancel (restock)

../common/               internal infra crate (no business logic):
                         auth (JWT/Argon2/RBAC extractor), error (RFC 7807),
                         db pool, http middleware+server, telemetry, pagination
```

## Tech stack

| Concern | Choice |
|---|---|
| HTTP | axum 0.8 + tower / tower-http |
| DB | PostgreSQL via sqlx 0.8 (compile-free runtime queries, pooled) |
| Cache | Redis (optional; degrades gracefully) |
| Auth | jsonwebtoken (HS256) + argon2 |
| Money | integer cents (`i64`) - never floats |
| Observability | tracing (JSON logs) + metrics + Prometheus exporter |

## Running

### Docker (everything)

```bash
docker compose up -d --build
curl http://localhost:8080/healthz          # -> ok
```

### Local (cargo)

```bash
cp .env.example .env
# start Postgres + Redis however you like, e.g.:
docker compose up -d postgres redis
cargo run -p server                          # migrations run on startup
```

## API

Base URL `http://localhost:8080`. All bodies are JSON. Errors are
`application/problem+json` (RFC 7807). Protected routes need
`Authorization: Bearer <access_token>`.

| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/v1/auth/register` | – | Create account, returns user + tokens |
| POST | `/v1/auth/login` | – | Exchange credentials for tokens |
| POST | `/v1/auth/refresh` | – | Rotate refresh token (reuse → family revoked) |
| POST | `/v1/auth/logout` | – | Revoke a refresh-token family |
| GET | `/v1/auth/me` | Yes | Current user profile |
| GET | `/v1/products` | – | List (paginated, `?q=`, `?category_id=`, `?cursor=`) |
| GET | `/v1/products/{id}` | – | Get one (cached) |
| POST | `/v1/products` | admin | Create |
| PATCH | `/v1/products/{id}` | admin | Partial update |
| DELETE | `/v1/products/{id}` | admin | Archive (soft delete) |
| GET | `/v1/categories` | – | List categories |
| POST | `/v1/categories` | admin | Create category |
| GET | `/v1/cart` | Yes | Get cart |
| POST | `/v1/cart/items` | Yes | Add item `{product_id, quantity}` |
| PATCH | `/v1/cart/items/{product_id}` | Yes | Set quantity |
| DELETE | `/v1/cart/items/{product_id}` | Yes | Remove line |
| DELETE | `/v1/cart` | Yes | Clear cart |
| POST | `/v1/orders` | Yes | Checkout cart → order |
| GET | `/v1/orders` | Yes | List my orders (paginated) |
| GET | `/v1/orders/{id}` | Yes | Order detail |
| POST | `/v1/orders/{id}/cancel` | Yes | Cancel + restock |
| GET | `/healthz` `/readyz` `/metrics` | – | Liveness / readiness / Prometheus |

### Example flow

```bash
# Register (returns tokens)
TOKEN=$(curl -s localhost:8080/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"email":"a@b.com","password":"hunter2hunter2","full_name":"Ada"}' \
  | jq -r .tokens.access_token)

# Browse, add to cart, checkout
PID=$(curl -s localhost:8080/v1/products | jq -r '.items[0].id')
curl -s localhost:8080/v1/cart/items -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d "{\"product_id\":\"$PID\",\"quantity\":2}"
curl -s -X POST localhost:8080/v1/orders -H "authorization: Bearer $TOKEN"
```

## Configuration

All via environment (see [.env.example](.env.example)). Key vars: `DATABASE_URL`,
`REDIS_URL` (optional), `JWT_ACCESS_SECRET`, `JWT_REFRESH_SECRET`,
`JWT_ACCESS_TTL_SECS`, `PORT`, `LOG_FORMAT`.

## Testing

```bash
cargo test --workspace      # unit tests: password hashing, JWT, RBAC, cursors
cargo clippy --all-targets -- -D warnings
```

## Production notes

- Secrets come from the environment; wire them to Vault/KMS in production.
- The in-process rate limiter and cache are single-node; back them with Redis
  for multi-replica deployments (the cache already uses Redis).
- Access tokens are short-lived (15 min) and verified locally; an optional Redis
  denylist can provide emergency revocation.
