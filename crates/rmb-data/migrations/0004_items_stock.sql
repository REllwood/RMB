-- 0004 — catalog (products & services) + append-only stock movements.

CREATE TABLE item (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    kind                TEXT NOT NULL DEFAULT 'product',   -- 'product' | 'service'
    name                TEXT NOT NULL,
    sku                 TEXT NOT NULL DEFAULT '',
    unit                TEXT NOT NULL DEFAULT 'each',
    default_price_minor INTEGER NOT NULL DEFAULT 0,
    default_tax_rate_id INTEGER REFERENCES tax_rate(id),
    tracked             INTEGER NOT NULL DEFAULT 0,        -- products only
    qty_on_hand         INTEGER NOT NULL DEFAULT 0,        -- cached = SUM(stock_movement.qty_delta)
    reorder_point       INTEGER,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at          TEXT
);

-- Append-only ledger; never updated/deleted (corrections are reversing rows).
CREATE TABLE stock_movement (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id     INTEGER NOT NULL REFERENCES item(id),
    qty_delta   INTEGER NOT NULL,
    reason      TEXT NOT NULL,                              -- receipt|sale|adjustment|return
    ref_type    TEXT,
    ref_id      INTEGER,
    occurred_at TEXT NOT NULL DEFAULT (datetime('now')),
    note        TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_stock_movement_item ON stock_movement (item_id);
