-- 0006 — payments + allocations (append-only).

CREATE TABLE payment (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id  INTEGER,
    date         TEXT NOT NULL DEFAULT (datetime('now')),
    amount_minor INTEGER NOT NULL,
    method       TEXT NOT NULL DEFAULT '',
    reference    TEXT NOT NULL DEFAULT ''
);

CREATE TABLE payment_allocation (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_id   INTEGER NOT NULL REFERENCES payment(id),
    invoice_id   INTEGER NOT NULL REFERENCES invoice(id),
    amount_minor INTEGER NOT NULL
);

CREATE INDEX idx_alloc_invoice ON payment_allocation (invoice_id);
