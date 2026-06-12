# Changelog

All notable changes to RMB. Format loosely follows [Keep a Changelog](https://keepachangelog.com).

## [Unreleased] — 0.1.0

First feature-complete build of the v1 core.

### The core loop
- Business settings with currency, **logo** (rendered on PDFs), document numbering, and a
  **configurable tax engine** (AU / NZ / UK / CA / US presets + custom rates, inclusive or exclusive).
- Customers with search and a **per-customer history** (quotes, jobs, invoices in one view).
- Catalog of products & services with **tracked stock** (append-only movement ledger, low-stock
  alerts) — items are picked directly onto documents, applying their default price + tax.
- Quotes: draft → sent → accepted, editable drafts, branded PDF, **convert to invoice or job**.
- Jobs & timekeeping: dated time entries (hours × rate) + materials; **invoice from job** bills
  un-invoiced work exactly once (atomic).
- Invoices: exact per-line integer-minor-unit tax math; due dates with **overdue** flags; editable
  and deletable drafts; **issue** assigns a gapless number, freezes business/customer snapshots, and
  decrements tracked stock exactly once; immutable once issued (**void + reissue** to correct).
- Payments: partial/full with method + reference, clamped overpayment, removable mis-entries
  (status re-derives), payment history per invoice.
- Dashboard: money owed, overdue, unpaid/draft/paid counts, low stock, recent invoices.
- PDF export via embedded Typst (offline, byte-identical across platforms).
- One-click safe backup (`VACUUM INTO`) and integrity-checked restore.

### Foundations
- Tauri 2 + React 19 desktop app, fully offline, data in one local SQLite file.
- Pure-Rust domain crate: money as integer minor units, property-tested tax/rounding,
  validated document/job status machines.
- WCAG 2.1 AA: keyboard operable, axe-tested screens, deterministic token-contrast tests,
  light + dark themes.
- 73 automated tests (51 Rust, 22 frontend); cargo audit/deny + npm audit clean.
