# Changelog

All notable changes to RMB. Format loosely follows [Keep a Changelog](https://keepachangelog.com).

## [Unreleased] — 0.1.0

First release candidate of the v1 core.

### Sales & work
- Business settings with currency, logo (rendered on PDFs), document numbering, and a configurable
  tax engine (AU / NZ / UK / CA / US presets plus custom rates, inclusive or exclusive).
- Customers with search and a per-customer history showing quotes, jobs, and invoices in one view.
- Catalog of products and services with tracked stock (append-only movement ledger, low-stock alerts).
  Items are picked directly onto documents, applying their default price and tax.
- Quotes: draft → sent → accepted, editable drafts, branded PDF, and conversion to an invoice or a job.
- Jobs and timekeeping: dated time entries (hours × rate) plus materials; invoicing a job bills
  un-invoiced work exactly once.
- Invoices: exact per-line tax on integer minor units; due dates with overdue flags; editable and
  deletable drafts. Issuing assigns a gapless number, freezes business and customer snapshots, and
  decrements tracked stock once. Issued invoices are immutable — void and reissue to correct.
- Recurring invoices: weekly through yearly schedules with optional end date and payment terms. Due
  schedules draft themselves at startup, catching up on any periods missed while the app was closed.
- Payments: partial or full with method and reference, overpayments rejected, mis-entries removable
  (status re-derives), and per-invoice payment history.

### Reporting & data
- Reports: tax collected per rate for any period, sales by month, and top customers, with preset and
  custom date ranges.
- CSV exports for invoices, payments, and customers.
- PDF export for invoices, quotes, and payment receipts, generated offline from an embedded template.
- Dashboard: money owed, overdue, unpaid/draft/paid counts, low stock, and recent invoices.
- Automatic backups on every launch (newest 7 plus one a day for a month), before every upgrade and
  before every restore, plus manual backup. A restore is validated, copied and upgraded before it
  replaces anything, and the outcome is reported at the next launch. Logos are embedded in SQLite so a
  database backup is portable without a separate asset folder.
- A second launch focuses the running window instead of opening the same database twice, and every
  commit is flushed to disk before it is reported as saved.

### Foundations
- Tauri 2 and React 19 desktop app, fully offline, with all data in one local SQLite file.
- Pure Rust domain crate: money as integer minor units, property-tested tax and rounding, and
  validated status machines for documents and jobs.
- WCAG 2.1 AA: keyboard operable, accessibility-tested screens, dialogs and forms, colour contrast
  checked from the stylesheet (including tinted surfaces, focus rings and field outlines), and light
  and dark themes.
- Money is typed exactly as entered (decimal points or commas, grouping), and every preview uses the
  same per-line rounding as the saved document.
- Voids are reported in their own period, drafts released by a void or delete can be billed again, and
  numbering skips numbers already used after a prefix change.
- Forms ask before unsaved changes are discarded, and simple forms submit with Enter.
- 223 automated tests (117 Rust, 106 front end), including repeated concurrency races on
  production-configured databases, a randomised invariants run, and an end-to-end pass over the full
  business flow. CI builds and tests the desktop crate too. Reviewed Rust advisory exceptions are
  documented in `deny.toml`.
