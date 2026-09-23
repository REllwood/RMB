-- 0012 — database-level uniqueness for identifiers the application treats as unique.
--
-- Builds before 0.1.0 did not enforce these rules, so an existing database can already hold
-- duplicates. Each block below resolves them deterministically (the oldest row keeps its value) and
-- records a notice for anything the user should review, before the index is created. Without this,
-- the index creation would fail and the app could never open the database again.

-- Tax rates: exact duplicates (same name, rate and inclusive flag) collapse onto the oldest active
-- row. Catalogue defaults are repointed first so no item is left on an archived rate.
UPDATE item SET default_tax_rate_id = (
    SELECT MIN(k.id) FROM tax_rate k, tax_rate d
    WHERE d.id = item.default_tax_rate_id AND k.archived = 0
      AND lower(k.name) = lower(d.name) AND k.rate_bp = d.rate_bp AND k.inclusive = d.inclusive)
WHERE default_tax_rate_id IN (
    SELECT d.id FROM tax_rate d WHERE d.archived = 0 AND EXISTS (
        SELECT 1 FROM tax_rate k WHERE k.archived = 0 AND k.id < d.id
          AND lower(k.name) = lower(d.name) AND k.rate_bp = d.rate_bp
          AND k.inclusive = d.inclusive));

UPDATE tax_rate SET archived = 1
WHERE archived = 0 AND EXISTS (
    SELECT 1 FROM tax_rate k WHERE k.archived = 0 AND k.id < tax_rate.id
      AND lower(k.name) = lower(tax_rate.name) AND k.rate_bp = tax_rate.rate_bp
      AND k.inclusive = tax_rate.inclusive);

-- Remaining active name clashes have different rates, so both are kept and the newer one is renamed.
INSERT INTO app_meta (key, value)
SELECT 'upgrade.notice.0012.tax_rates',
       'Tax rates with the same name were renamed so each is unique: '
       || group_concat(name || ' → ' || name || ' (' || id || ')', ', ')
       || '. Review them in Settings.'
FROM tax_rate
WHERE archived = 0 AND EXISTS (
    SELECT 1 FROM tax_rate k WHERE k.archived = 0 AND k.id < tax_rate.id
      AND lower(k.name) = lower(tax_rate.name))
HAVING COUNT(*) > 0;

UPDATE tax_rate SET name = name || ' (' || id || ')'
WHERE archived = 0 AND EXISTS (
    SELECT 1 FROM tax_rate k WHERE k.archived = 0 AND k.id < tax_rate.id
      AND lower(k.name) = lower(tax_rate.name));

-- SKUs: the oldest active item keeps the SKU; later duplicates get their id appended.
INSERT INTO app_meta (key, value)
SELECT 'upgrade.notice.0012.skus',
       'Duplicate SKUs were made unique: '
       || group_concat(sku || ' → ' || sku || '-' || id, ', ')
       || '. Review them in the Catalog.'
FROM item
WHERE deleted_at IS NULL AND trim(sku) <> '' AND EXISTS (
    SELECT 1 FROM item k WHERE k.deleted_at IS NULL AND k.id < item.id
      AND lower(k.sku) = lower(item.sku))
HAVING COUNT(*) > 0;

UPDATE item SET sku = sku || '-' || id
WHERE deleted_at IS NULL AND trim(sku) <> '' AND EXISTS (
    SELECT 1 FROM item k WHERE k.deleted_at IS NULL AND k.id < item.id
      AND lower(k.sku) = lower(item.sku));

-- Document numbers: a numbering-prefix change could reuse a number. The first document keeps it and
-- later ones are marked so they stay distinguishable on screen and in exports.
INSERT INTO app_meta (key, value)
SELECT 'upgrade.notice.0012.invoice_numbers',
       'Some invoices shared a number and were renamed: '
       || group_concat(number || ' → ' || number || '-DUP' || id, ', ')
       || '. Check what was sent to those customers.'
FROM invoice
WHERE number IS NOT NULL AND EXISTS (
    SELECT 1 FROM invoice k WHERE k.number = invoice.number AND k.id < invoice.id)
HAVING COUNT(*) > 0;

UPDATE invoice SET number = number || '-DUP' || id
WHERE number IS NOT NULL AND EXISTS (
    SELECT 1 FROM invoice k WHERE k.number = invoice.number AND k.id < invoice.id);

UPDATE quote SET number = number || '-DUP' || id
WHERE number IS NOT NULL AND EXISTS (
    SELECT 1 FROM quote k WHERE k.number = quote.number AND k.id < quote.id);

-- Conversion links: a double-click could convert the same quote or job twice. The first document
-- keeps the link; later ones remain as ordinary documents without it.
UPDATE invoice SET source_quote_id = NULL
WHERE source_quote_id IS NOT NULL AND EXISTS (
    SELECT 1 FROM invoice k WHERE k.source_quote_id = invoice.source_quote_id AND k.id < invoice.id);

UPDATE invoice SET source_job_id = NULL
WHERE source_job_id IS NOT NULL AND EXISTS (
    SELECT 1 FROM invoice k WHERE k.source_job_id = invoice.source_job_id AND k.id < invoice.id);

UPDATE job SET source_quote_id = NULL
WHERE source_quote_id IS NOT NULL AND EXISTS (
    SELECT 1 FROM job k WHERE k.source_quote_id = job.source_quote_id AND k.id < job.id);

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
