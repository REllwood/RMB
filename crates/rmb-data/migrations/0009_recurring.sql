-- 0009 — recurring invoice schedules. run_due() turns due schedules into ordinary draft
-- invoices (one per elapsed period), advancing next_date atomically with each draft.

CREATE TABLE recurring_invoice (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id  INTEGER NOT NULL REFERENCES customer(id),
    frequency    TEXT NOT NULL,                           -- weekly|fortnightly|monthly|quarterly|yearly
    next_date    TEXT NOT NULL,                           -- next generation date (YYYY-MM-DD)
    end_date     TEXT,                                    -- last date to generate (inclusive); NULL = forever
    due_days     INTEGER,                                 -- generated invoice due = generation date + due_days
    notes        TEXT NOT NULL DEFAULT '',
    active       INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE recurring_invoice_line (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    recurring_id     INTEGER NOT NULL REFERENCES recurring_invoice(id),
    item_id          INTEGER,
    description      TEXT NOT NULL,
    quantity         TEXT NOT NULL,                       -- exact decimal as string
    unit_price_minor INTEGER NOT NULL,
    tax_rate_name    TEXT NOT NULL,
    tax_rate_bp      INTEGER NOT NULL,
    tax_inclusive    INTEGER NOT NULL,
    line_order       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_recurring_customer ON recurring_invoice (customer_id);
CREATE INDEX idx_recurring_line ON recurring_invoice_line (recurring_id);
