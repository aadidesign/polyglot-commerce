-- ---- Catalog: categories & products (with in-line stock) ----
CREATE TABLE IF NOT EXISTS categories (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name       TEXT NOT NULL,
    slug       TEXT NOT NULL UNIQUE,
    parent_id  UUID REFERENCES categories(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS products (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sku            TEXT NOT NULL UNIQUE,
    name           TEXT NOT NULL,
    slug           TEXT NOT NULL UNIQUE,
    description    TEXT NOT NULL DEFAULT '',
    price_cents    BIGINT NOT NULL CHECK (price_cents >= 0),
    currency       TEXT NOT NULL DEFAULT 'USD',
    stock_quantity INTEGER NOT NULL DEFAULT 0 CHECK (stock_quantity >= 0),
    category_id    UUID REFERENCES categories(id) ON DELETE SET NULL,
    status         TEXT NOT NULL DEFAULT 'active'
                   CHECK (status IN ('active', 'archived')),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    search_vector  TSVECTOR GENERATED ALWAYS AS (
        to_tsvector('english', coalesce(name, '') || ' ' || coalesce(description, ''))
    ) STORED
);

CREATE INDEX IF NOT EXISTS idx_products_search   ON products USING GIN (search_vector);
CREATE INDEX IF NOT EXISTS idx_products_keyset    ON products (created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_products_category  ON products (category_id);

-- ---- Seed sample catalog ----
INSERT INTO categories (id, name, slug) VALUES
    ('11111111-1111-1111-1111-111111111111', 'Electronics', 'electronics'),
    ('22222222-2222-2222-2222-222222222222', 'Books', 'books'),
    ('33333333-3333-3333-3333-333333333333', 'Home & Kitchen', 'home-kitchen')
ON CONFLICT (slug) DO NOTHING;

INSERT INTO products (sku, name, slug, description, price_cents, stock_quantity, category_id) VALUES
    ('ELEC-1001', 'Mechanical Keyboard', 'mechanical-keyboard',
     'Hot-swappable RGB mechanical keyboard with PBT keycaps.', 12900, 50,
     '11111111-1111-1111-1111-111111111111'),
    ('ELEC-1002', 'Noise-Cancelling Headphones', 'noise-cancelling-headphones',
     'Over-ear wireless headphones with active noise cancellation.', 24900, 30,
     '11111111-1111-1111-1111-111111111111'),
    ('BOOK-2001', 'Designing Data-Intensive Applications', 'ddia',
     'The big ideas behind reliable, scalable, maintainable systems.', 4500, 200,
     '22222222-2222-2222-2222-222222222222'),
    ('HOME-3001', 'Pour-Over Coffee Maker', 'pour-over-coffee-maker',
     'Borosilicate glass pour-over set for a clean, bright cup.', 3200, 75,
     '33333333-3333-3333-3333-333333333333')
ON CONFLICT (sku) DO NOTHING;
