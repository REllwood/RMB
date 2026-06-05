-- 0002 — business settings (singleton) + configurable tax rates.

CREATE TABLE settings (
    id                   INTEGER PRIMARY KEY CHECK (id = 1),
    business_name        TEXT NOT NULL DEFAULT '',
    address              TEXT NOT NULL DEFAULT '',
    email                TEXT NOT NULL DEFAULT '',
    phone                TEXT NOT NULL DEFAULT '',
    logo_path            TEXT,
    currency             TEXT NOT NULL DEFAULT 'USD',
    tax_label            TEXT NOT NULL DEFAULT 'Tax',
    tax_number           TEXT NOT NULL DEFAULT '',
    prices_tax_inclusive INTEGER NOT NULL DEFAULT 0,
    invoice_prefix       TEXT NOT NULL DEFAULT 'INV-',
    invoice_next_seq     INTEGER NOT NULL DEFAULT 1,
    quote_prefix         TEXT NOT NULL DEFAULT 'Q-',
    quote_next_seq       INTEGER NOT NULL DEFAULT 1,
    number_pad           INTEGER NOT NULL DEFAULT 4
);

INSERT INTO settings (id) VALUES (1);

CREATE TABLE tax_rate (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    name      TEXT NOT NULL,
    rate_bp   INTEGER NOT NULL,
    inclusive INTEGER NOT NULL DEFAULT 0,
    archived  INTEGER NOT NULL DEFAULT 0
);
