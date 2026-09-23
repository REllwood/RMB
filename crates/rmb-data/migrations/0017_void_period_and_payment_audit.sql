-- 0017 — report voids in the period they happen, and keep an audit record of removed payments.
--
-- An invoice counts in the period it was issued. When it is later voided, the reversal belongs to
-- the period of the void (like a credit note), so a period that has already been reported never
-- changes after the fact.

ALTER TABLE invoice ADD COLUMN void_date TEXT;

UPDATE invoice SET void_date = COALESCE(date(voided_at, 'localtime'), issue_date)
WHERE status = 'void';

-- A removed payment disappears from balances and cash reports; this keeps what was removed.
CREATE TABLE payment_removal (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_id   INTEGER NOT NULL,
    invoice_id   INTEGER,
    customer_id  INTEGER,
    date         TEXT NOT NULL,
    amount_minor INTEGER NOT NULL,
    method       TEXT NOT NULL,
    reference    TEXT NOT NULL,
    removed_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_invoice_issue_date ON invoice (issue_date);
CREATE INDEX idx_invoice_void_date ON invoice (void_date) WHERE void_date IS NOT NULL;
CREATE INDEX idx_invoice_status_due ON invoice (status, due_date);
CREATE INDEX idx_payment_allocation_payment ON payment_allocation (payment_id);
