# Changelog

All notable changes to RMB. Format loosely follows [Keep a Changelog](https://keepachangelog.com).

## [Unreleased] — 0.1.0

First feature-complete release of the v1 core.

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
- Payments: partial or full with method and reference, overpayment clamped, mis-entries removable
  (status re-derives), and per-invoice payment history.

### Reporting & data
- Reports: tax collected per rate for any period, sales by month, and top customers, with preset and
  custom date ranges.
- CSV exports for invoices, payments, and customers.
- PDF export for invoices, quotes, and payment receipts — generated offline, identical across platforms.
- Dashboard: money owed, overdue, unpaid/draft/paid counts, low stock, and recent invoices.
- Rotating automatic backup on every launch (newest 7 kept), plus manual backup and an
  integrity-checked restore.

### Foundations
- Tauri 2 and React 19 desktop app, fully offline, with all data in one local SQLite file.
- Pure Rust domain crate: money as integer minor units, property-tested tax and rounding, and
  validated status machines for documents and jobs.
- WCAG 2.1 AA: keyboard operable, accessibility-tested screens, deterministic colour-contrast tests,
  and light and dark themes.
- 80 automated tests (57 Rust, 23 front end), including an end-to-end pass over the full business
  flow. Dependency audits clean.
