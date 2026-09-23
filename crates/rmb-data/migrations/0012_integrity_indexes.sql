-- 0012 — database-level uniqueness for identifiers the application treats as unique.

CREATE UNIQUE INDEX idx_invoice_number_unique
    ON invoice (number) WHERE number IS NOT NULL;

CREATE UNIQUE INDEX idx_quote_number_unique
    ON quote (number) WHERE number IS NOT NULL;

CREATE UNIQUE INDEX idx_item_active_sku_unique
    ON item (lower(sku)) WHERE deleted_at IS NULL AND trim(sku) <> '';

CREATE UNIQUE INDEX idx_tax_rate_active_name_unique
    ON tax_rate (lower(name)) WHERE archived = 0;

CREATE UNIQUE INDEX idx_invoice_source_quote_unique
    ON invoice (source_quote_id) WHERE source_quote_id IS NOT NULL;

CREATE UNIQUE INDEX idx_invoice_source_job_unique
    ON invoice (source_job_id) WHERE source_job_id IS NOT NULL;

CREATE UNIQUE INDEX idx_job_source_quote_unique
    ON job (source_quote_id) WHERE source_quote_id IS NOT NULL;
