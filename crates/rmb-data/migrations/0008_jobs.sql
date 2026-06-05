-- 0008 — jobs with time entries + materials. Jobs don't touch stock; invoicing a job creates a
-- draft invoice (labour from time, products from materials) and stock decrements at invoice issue.

CREATE TABLE job (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id     INTEGER NOT NULL REFERENCES customer(id),
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'open', -- open|in_progress|done|invoiced
    source_quote_id INTEGER,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at      TEXT
);

CREATE TABLE time_entry (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id        INTEGER NOT NULL REFERENCES job(id),
    date          TEXT NOT NULL DEFAULT (date('now')),
    minutes       INTEGER NOT NULL,
    rate_minor    INTEGER NOT NULL DEFAULT 0, -- per hour
    description   TEXT NOT NULL DEFAULT '',
    tax_rate_name TEXT NOT NULL DEFAULT 'No Tax',
    tax_rate_bp   INTEGER NOT NULL DEFAULT 0,
    tax_inclusive INTEGER NOT NULL DEFAULT 0,
    invoiced      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE job_material (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id           INTEGER NOT NULL REFERENCES job(id),
    item_id          INTEGER,
    description      TEXT NOT NULL,
    quantity         TEXT NOT NULL,
    unit_price_minor INTEGER NOT NULL,
    tax_rate_name    TEXT NOT NULL DEFAULT 'No Tax',
    tax_rate_bp      INTEGER NOT NULL DEFAULT 0,
    tax_inclusive    INTEGER NOT NULL DEFAULT 0,
    invoiced         INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_time_entry_job ON time_entry (job_id);
CREATE INDEX idx_job_material_job ON job_material (job_id);
