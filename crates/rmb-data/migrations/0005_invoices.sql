-- 0005 — invoices (immutable once issued) + snapshotted line items.

CREATE TABLE invoice (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id       INTEGER NOT NULL REFERENCES customer(id),
    source_quote_id   INTEGER,
    source_job_id     INTEGER,
    number            TEXT,                                  -- assigned at issue (gapless)
    status            TEXT NOT NULL DEFAULT 'draft',         -- draft|issued|part_paid|paid|void
    issue_date        TEXT,
    due_date          TEXT,
    business_snapshot TEXT,                                  -- JSON, frozen at issue
    customer_snapshot TEXT,                                  -- JSON, frozen at issue
    subtotal_minor    INTEGER NOT NULL DEFAULT 0,
    tax_minor         INTEGER NOT NULL DEFAULT 0,
    total_minor       INTEGER NOT NULL DEFAULT 0,
    tax_summary       TEXT NOT NULL DEFAULT '[]',            -- JSON RateSummary[]
    notes             TEXT NOT NULL DEFAULT '',
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    issued_at         TEXT,
    voided_at         TEXT
);

CREATE TABLE invoice_line (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id       INTEGER NOT NULL REFERENCES invoice(id),
    item_id          INTEGER,
    description      TEXT NOT NULL,
    quantity         TEXT NOT NULL,                          -- exact decimal as string
    unit_price_minor INTEGER NOT NULL,
    tax_rate_name    TEXT NOT NULL,
    tax_rate_bp      INTEGER NOT NULL,
    tax_inclusive    INTEGER NOT NULL,
    net_minor        INTEGER NOT NULL,
    tax_minor        INTEGER NOT NULL,
    gross_minor      INTEGER NOT NULL,
    line_order       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_invoice_customer ON invoice (customer_id);
CREATE INDEX idx_invoice_line_invoice ON invoice_line (invoice_id);
