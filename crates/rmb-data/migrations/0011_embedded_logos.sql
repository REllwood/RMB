-- 0011 — keep backups self-contained and issued-document branding immutable.
-- Paths remain for backwards compatibility and UI display. Each version is stored once; settings
-- points at the current asset and issued invoices retain that immutable asset ID.

CREATE TABLE business_logo_asset (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    format     TEXT NOT NULL CHECK (format IN ('png', 'jpg')),
    data       BLOB NOT NULL CHECK (length(data) > 0),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

ALTER TABLE settings
    ADD COLUMN logo_asset_id INTEGER REFERENCES business_logo_asset(id);

ALTER TABLE invoice
    ADD COLUMN business_logo_asset_id INTEGER REFERENCES business_logo_asset(id);

CREATE INDEX idx_invoice_business_logo_asset ON invoice (business_logo_asset_id);
