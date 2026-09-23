-- 0016 — the tax rate new document lines start with. NULL falls back to the first non-zero rate.

ALTER TABLE settings
    ADD COLUMN default_tax_rate_id INTEGER REFERENCES tax_rate(id);
