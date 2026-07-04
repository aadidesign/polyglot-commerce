# E-Commerce Backend — One API, Three Languages

A production-grade **e-commerce REST API** — authentication, product catalog,
cart, and orders — implemented **three independent times**, once each in
**Rust**, **Go**, and **Python**, against a **single shared contract**.

Each folder is a complete, self-contained, runnable backend with its own
database schema, migrations, Docker image, tests, and README. They expose the
**same endpoints and JSON shapes** ([`api/openapi.yaml`](api/openapi.yaml)), so
they are drop-in interchangeable — and directly comparable.

> Why build the same thing three times? Because the interesting part of backend
> engineering is not "can you do CRUD" — it's the *cross-cutting* decisions:
> auth, concurrency-safe checkout, pagination at scale, error contracts,
> observability, and clean layering. This repo shows those decisions made
> idiomatically in three ecosystems.

```
ECommerce/
├── api/openapi.yaml     # the shared REST contract all three honor
├── rust/                # axum + sqlx + tokio        →  rust/README.md
├── go/                  # chi + pgx                  →  go/README.md
└── python/              # FastAPI + asyncpg          →  python/README.md
```

---

## What every implementation includes

| Capability | Detail |
|---|---|
| **Authentication** | Registration, login, **Argon2id** password hashing |
| **Tokens** | JWT **access (15m) + refresh (30d)** with **rotation & reuse detection** |
| **Authorization** | **RBAC** — roles → permissions, enforced per route |
| **Catalog** | Product & category CRUD, **Postgres full-text search** |
| **Pagination** | **Opaque keyset (cursor)** pagination — no `OFFSET` on hot paths |
| **Caching** | **Redis** read-through for products (optional, degrades gracefully) |
| **Cart** | Durable, per-user, add/update/remove/clear |
| **Checkout** | **Single ACID transaction** with `SELECT … FOR UPDATE` — *no oversell* |
| **Orders** | List (paginated), detail, **cancel with restock** |
| **Errors** | **RFC 7807** `application/problem+json` everywhere |
| **Observability** | **Prometheus** `/metrics`, structured **JSON logs**, request IDs |
| **Health** | `/healthz` (liveness) + `/readyz` (readiness, checks DB) |
| **Lifecycle** | Config from env (12-Factor), **graceful shutdown**, DB **migrations** |
| **Delivery** | Multi-stage **Dockerfile** + **docker-compose** (Postgres + Redis + API) |
| **Tests** | Unit tests for hashing + JWT/RBAC; CI builds & tests all three |

## The checkout invariant (the hard part)

In all three, checkout is a single database transaction that **locks the
relevant product rows** (`SELECT … FOR UPDATE`), verifies stock, decrements it,
snapshots line prices, and clears the cart — atomically. Two shoppers racing for
the last unit cannot both succeed; one gets a `409 Conflict`. Cancellation
compensates by restocking inside its own transaction.

This is deliberately a **modular monolith per service**, not a distributed saga:
when one service owns the data, an ACID transaction is simpler *and* stronger
than eventual consistency. (An earlier event-driven microservices iteration
informed this choice — the right tool depends on whether data is co-located.)

## Implementation matrix

| Concern | Rust | Go | Python |
|---|---|---|---|
| HTTP framework | axum 0.8 + tower | go-chi/chi v5 | FastAPI (ASGI) |
| DB driver | sqlx 0.8 (pooled) | jackc/pgx v5 | asyncpg |
| Passwords | argon2 crate | x/crypto/argon2 | argon2-cffi |
| JWT | jsonwebtoken | golang-jwt/v5 | PyJWT |
| Validation | validator | hand-rolled | Pydantic v2 |
| Logging | tracing (JSON) | log/slog (JSON) | logging (JSON) |
| Metrics | metrics + Prom exporter | client_golang | prometheus-client |
| Runtime image | distroless-ish debian-slim | distroless static | python:slim |
| Money | `i64` cents | `int64` cents | `int` cents |

## Quick start

Pick any implementation — they behave identically. Each is fully self-contained:

```bash
cd rust      # or: cd go   |   cd python
docker compose up -d --build
curl http://localhost:8080/healthz        # -> ok
```

End-to-end smoke (works against any of the three):

```bash
# 1) Register → capture the access token
TOKEN=$(curl -s localhost:8080/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"email":"a@b.com","password":"hunter2hunter2","full_name":"Ada"}' \
  | jq -r .tokens.access_token)

# 2) Browse the seeded catalog → grab a product id
PID=$(curl -s localhost:8080/v1/products | jq -r '.items[0].id')

# 3) Add to cart and check out
curl -s localhost:8080/v1/cart/items -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d "{\"product_id\":\"$PID\",\"quantity\":2}"
curl -s -X POST localhost:8080/v1/orders -H "authorization: Bearer $TOKEN" | jq
```

Each backend seeds a small catalog on first run, so the API returns data
immediately. The Python build additionally serves Swagger UI at `/docs`.

## API reference

The authoritative contract is [`api/openapi.yaml`](api/openapi.yaml) (OpenAPI 3.1).

| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/v1/auth/register` | – | Create account → user + tokens |
| POST | `/v1/auth/login` | – | Credentials → tokens |
| POST | `/v1/auth/refresh` | – | Rotate refresh token (reuse → family revoked) |
| POST | `/v1/auth/logout` | – | Revoke a refresh-token family |
| GET | `/v1/auth/me` | Yes | Current user |
| GET | `/v1/products` | – | List — `?q=`, `?category_id=`, `?cursor=`, `?limit=` |
| GET | `/v1/products/{id}` | – | Get one (cached) |
| POST·PATCH·DELETE | `/v1/products[/{id}]` | admin | Create / update / archive |
| GET·POST | `/v1/categories` | – / admin | List / create |
| GET·DELETE | `/v1/cart` | Yes | Get / clear cart |
| POST | `/v1/cart/items` | Yes | Add item |
| PATCH·DELETE | `/v1/cart/items/{product_id}` | Yes | Set qty / remove |
| POST·GET | `/v1/orders` | Yes | Checkout / list orders |
| GET | `/v1/orders/{id}` | Yes | Order detail |
| POST | `/v1/orders/{id}/cancel` | Yes | Cancel + restock |
| GET | `/healthz` · `/readyz` · `/metrics` | – | Ops endpoints |

The `admin` role and its permissions are seeded; the default registration grants
the `customer` role. Promote a user by inserting into `user_roles` (see each
implementation's `0001` migration for the seeded RBAC).

## Project layout

```
api/openapi.yaml          shared contract (single source of truth)
rust/   common/  + server/ (auth · products · cart · orders)   · migrations · Dockerfile · compose
go/     cmd/server + internal/{auth,catalog,cart,orders,db,httpx,pagination,server,config}
python/ app/{routers,...}  · migrations · Dockerfile · compose
.github/workflows/ci.yml   builds + tests all three on every push
```

## Testing

```bash
cd rust   && cargo test --workspace          # 7 unit tests
cd go     && go test ./...                    # argon2 + JWT/RBAC
cd python && pip install -e ".[dev]" && pytest
```

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs `fmt/clippy +
test` (Rust), `vet + build + test` (Go), and `ruff + pytest` (Python).

## What this repository demonstrates

- **Polyglot fluency** — idiomatic, not transliterated, code in three ecosystems.
- **Security done right** — Argon2id, refresh-token rotation *with reuse
  detection*, RBAC, least-privilege tokens.
- **Correctness under concurrency** — row-locked, oversell-proof checkout.
- **Scale-aware design** — keyset pagination, connection pooling, read-through
  caching, full-text search.
- **Operability** — RFC 7807 errors, Prometheus metrics, structured logs, health
  probes, graceful shutdown, migrations, reproducible Docker builds.
- **Engineering judgment** — clear bounded modules, a shared contract, ADR-style
  reasoning in the READMEs, and choosing ACID-over-saga when the data allows.

## License

MIT — see individual modules. Built as a backend engineering portfolio piece.
