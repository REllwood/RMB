-- 0013 — repair line data that builds before 0.1.0 stored without validation.
--
-- Quote lines, recurring templates and job materials are re-validated when they are converted,
-- generated or billed. Older builds accepted blank descriptions, zero or negative quantities, and
-- free-text material quantities such as "1,5", which would make those records impossible to use.
-- Every repair below keeps the line's amount (unit price × quantity) exactly the same.

-- Free-text material quantities: surrounding spaces, and a single decimal comma ("1,5" → "1.5").
UPDATE job_material SET quantity = trim(quantity) WHERE quantity <> trim(quantity);
UPDATE job_material SET quantity = replace(quantity, ',', '.')
WHERE instr(quantity, ',') > 0 AND instr(quantity, '.') = 0
  AND length(quantity) - length(replace(quantity, ',', '')) = 1;

-- Negative quantities become positive quantities of a negative (discount) price.
UPDATE quote_line SET quantity = substr(trim(quantity), 2), unit_price_minor = -unit_price_minor
WHERE trim(quantity) GLOB '-[0-9]*' AND substr(trim(quantity), 2) NOT GLOB '*[^0-9.]*';
UPDATE recurring_invoice_line
SET quantity = substr(trim(quantity), 2), unit_price_minor = -unit_price_minor
WHERE trim(quantity) GLOB '-[0-9]*' AND substr(trim(quantity), 2) NOT GLOB '*[^0-9.]*';
UPDATE job_material SET quantity = substr(trim(quantity), 2), unit_price_minor = -unit_price_minor
WHERE trim(quantity) GLOB '-[0-9]*' AND substr(trim(quantity), 2) NOT GLOB '*[^0-9.]*';

-- Zero quantities contribute nothing, so they become one unit at a zero price.
UPDATE quote_line SET quantity = '1', unit_price_minor = 0
WHERE trim(quantity) GLOB '*[0-9]*' AND trim(quantity) NOT GLOB '*[^0-9.]*'
  AND CAST(trim(quantity) AS REAL) = 0;
UPDATE recurring_invoice_line SET quantity = '1', unit_price_minor = 0
WHERE trim(quantity) GLOB '*[0-9]*' AND trim(quantity) NOT GLOB '*[^0-9.]*'
  AND CAST(trim(quantity) AS REAL) = 0;
UPDATE job_material SET quantity = '1', unit_price_minor = 0
WHERE trim(quantity) GLOB '*[0-9]*' AND trim(quantity) NOT GLOB '*[^0-9.]*'
  AND CAST(trim(quantity) AS REAL) = 0;

-- Blank and over-long descriptions.
UPDATE quote_line SET description = '(no description)' WHERE trim(description) = '';
UPDATE recurring_invoice_line SET description = '(no description)' WHERE trim(description) = '';
UPDATE job_material SET description = '(no description)' WHERE trim(description) = '';
UPDATE quote_line SET description = substr(description, 1, 2000) WHERE length(description) > 2000;
UPDATE recurring_invoice_line SET description = substr(description, 1, 2000)
WHERE length(description) > 2000;
UPDATE job_material SET description = substr(description, 1, 2000) WHERE length(description) > 2000;
