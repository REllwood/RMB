-- 0014 — a conversion link only has to be unique among live records. Voiding an invoice or deleting
-- a job releases the quote or job it came from, so the same work can be billed again while the
-- voided or deleted record keeps its history.

DROP INDEX idx_invoice_source_quote_unique;
DROP INDEX idx_invoice_source_job_unique;
DROP INDEX idx_job_source_quote_unique;

CREATE UNIQUE INDEX idx_invoice_source_quote_unique
    ON invoice (source_quote_id) WHERE source_quote_id IS NOT NULL AND status <> 'void';

CREATE UNIQUE INDEX idx_invoice_source_job_unique
    ON invoice (source_job_id) WHERE source_job_id IS NOT NULL AND status <> 'void';

CREATE UNIQUE INDEX idx_job_source_quote_unique
    ON job (source_quote_id) WHERE source_quote_id IS NOT NULL AND deleted_at IS NULL;
