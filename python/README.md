# 🐍 E-Commerce Backend — Python

A production-grade e-commerce REST API built with **FastAPI + asyncpg**. One of
three independent implementations of the same contract (see the
[root README](../README.md) for the Rust and Go siblings and a comparison).

## Highlights

- **Auth** — registration, login, **Argon2id** hashing (`argon2-cffi`), JWT
  **access + refresh tokens with rotation & reuse detection** (PyJWT), **RBAC**
  via dependency injection.
- **Catalog** — product/category CRUD, **Postgres full-text search**, **keyset
  (cursor) pagination**, optional **Redis** read-through caching.
- **Cart** — durable per-user cart.
- **Orders** — checkout as a **single ACID transaction** with `SELECT … FOR
  UPDATE` row locking (no oversell), price snapshotting, restock-on-cancel.
- **Cross-cutting** — RFC 7807 `problem+json` errors via exception handlers,
  Pydantic request validation, structured JSON logging, **Prometheus**
  `/metrics`, CORS, async lifespan management, embedded SQL migrator.

## Layout

```
app/
├── main.py            FastAPI factory + lifespan (db/redis), middleware, health/metrics
├── config.py          pydantic-settings configuration
├── db.py              asyncpg pool + embedded-SQL migrator
├── security.py        Argon2id + JWT manager
├── errors.py          RFC 7807 APIError + exception handlers
├── deps.py            DI: pool, jwt, current user, require_permission (RBAC)
├── pagination.py      keyset cursor helpers
├── schemas.py         Pydantic request models
├── observability.py   JSON logging + Prometheus middleware
└── routers/           auth · products · cart · orders
migrations/*.sql       applied on startup by the embedded migrator
```

## Running

### Docker

```bash
docker compose up -d --build
curl http://localhost:8080/healthz
```

### Local

```bash
python -m venv .venv && . .venv/bin/activate    # Windows: .venv\Scripts\activate
pip install -r requirements.txt
cp .env.example .env
docker compose up -d postgres redis
uvicorn app.main:app --reload
```

Interactive API docs are served at **`/docs`** (Swagger UI) and **`/redoc`**.

## API

Identical contract to the other implementations — see the
[OpenAPI spec](../api/openapi.yaml) and the endpoint table in the
[root README](../README.md). Base URL `http://localhost:8080`.

## Testing

```bash
pip install -e ".[dev]"
pytest          # Argon2 + JWT/RBAC unit tests
ruff check .
```

## Tech stack

| Concern | Choice |
|---|---|
| Framework | FastAPI (ASGI, async) |
| Server | uvicorn |
| DB | PostgreSQL via asyncpg (pooled) |
| Cache | redis.asyncio (optional) |
| Auth | PyJWT + argon2-cffi |
| Validation | Pydantic v2 |
| Metrics | prometheus-client |
| Money | integer cents (`int`) |
