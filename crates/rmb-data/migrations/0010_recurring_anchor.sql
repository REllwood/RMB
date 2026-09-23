-- 0010 — preserve the intended day-of-month for monthly, quarterly, and yearly schedules.
-- Without an anchor, SQLite's '+1 month' can turn 31 January into 3 March and permanently drift.

ALTER TABLE recurring_invoice
    ADD COLUMN anchor_day INTEGER NOT NULL DEFAULT 1 CHECK (anchor_day BETWEEN 1 AND 31);

UPDATE recurring_invoice
SET anchor_day = COALESCE(CAST(strftime('%d', next_date) AS INTEGER), 1);
