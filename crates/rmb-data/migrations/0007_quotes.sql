-- 0007 — quotes (estimates). Like invoices but no stock/payments; can convert to an invoice.

CREATE TABLE quote (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id          INTEGER NOT NULL REFERENCES customer(id),
    number               TEXT,
    status               TEXT NOT NULL DEFAULT 'draft', -- draft|sent|accepted|declined|expired|converted
    valid_until          TEXT,
    subtotal_minor       INTEGER NOT NULL DEFAULT 0,
    tax_minor            INTEGER NOT NULL DEFAULT 0,
    total_minor          INTEGER NOT NULL DEFAULT 0,
    tax_summary          TEXT NOT NULL DEFAULT '[]',
    notes                TEXT NOT NULL DEFAULT '',
    converted_invoice_id INTEGER,
    created_at           TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at           TEXT
);

CREATE TABLE quote_line (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    quote_id         INTEGER NOT NULL REFERENCES quote(id),
    item_id          INTEGER,
    description      TEXT NOT NULL,
    quantity         TEXT NOT NULL,
    unit_price_minor INTEGER NOT NULL,
    tax_rate_name    TEXT NOT NULL,
    tax_rate_bp      INTEGER NOT NULL,
    tax_inclusive    INTEGER NOT NULL,
    net_minor        INTEGER NOT NULL,
    tax_minor        INTEGER NOT NULL,
    gross_minor      INTEGER NOT NULL,
    line_order       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_quote_customer ON quote (customer_id);
CREATE INDEX idx_quote_line_quote ON quote_line (quote_id);
