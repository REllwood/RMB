-- 0018 — default payment terms. A new draft without a due date gets these terms, so when it is
-- issued its due date is the issue date plus this many days (and it can become overdue).

ALTER TABLE settings
    ADD COLUMN default_due_days INTEGER
        CHECK (default_due_days IS NULL OR default_due_days BETWEEN 0 AND 3650);
