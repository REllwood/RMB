# RMB — business manager for small & home businesses

A free, open-source desktop app that gives a small business one place to run the core loop —
customers, catalog, stock, quotes, invoices, payments — with **all data on your own machine**.
No subscriptions, no cloud, works fully offline.

Most small businesses end up paying £20–50 a month, per user, for a patchwork of tools that each do
one part of this. RMB is an attempt to cover the whole loop in a single app you download once and own.

> **Status: v1 release candidate.** The core workflows below are implemented and pass the automated
> release gate. Public deployment still requires a signed and notarised macOS build plus installer
> testing on clean Windows and macOS machines.

## What it does

- **Business setup** — your details, logo (shown on PDFs), currency, and a configurable tax engine
  with one-click presets for the UK (VAT), Australia / New Zealand (GST), Canada, and the US, plus
  custom rates.
- **Customers** — searchable records, each with a full history: every quote, job, and invoice in one
  place.
- **Catalog & inventory** — products (with tracked stock) and services, backed by an append-only stock
  ledger with low-stock alerts and a per-item stock history. Catalog items are picked straight onto
  quotes, jobs, and invoices with their default price and tax, so selling actually moves stock.
  Archived items and tax rates can be restored.
- **Quotes** — estimates with the same exact tax; edit drafts, send, accept, then convert into an
  invoice or a job in one click.
- **Jobs & timekeeping** — track dated time entries (hours × rate) and materials against a job, then
  create an invoice from the job in one step. Billed work can't be pulled twice.
- **Invoices** — built from catalog items or free lines, with exact per-line tax (inclusive or
  exclusive, multiple rates), due dates, and overdue flags. Drafts are editable and deletable; issuing
  assigns a gapless number, freezes a snapshot of your and the customer's details, and decrements stock
  exactly once. Issued invoices are immutable — corrections go through void + reissue.
- **Recurring invoices** — weekly, fortnightly, monthly, quarterly, or yearly schedules that draft
  themselves when due (including catching up after time away). You still review and issue each one.
- **Payments** — record partial or full payments with method and reference; status flows unpaid →
  part-paid → paid with a live balance. A mis-entered payment can be removed and the status recalculates,
  and any invoice with payments can produce a receipt PDF.
- **Reports & exports** — tax collected per rate for any period, sales by month and by customer, plus
  CSV exports of invoices, payments, and customers for your accountant. A void is reported in the
  period it happens, so a period you've already reported never changes. These are operational reports,
  not tax filing or accounting advice.
- **PDF export** — branded invoice, quote, and receipt PDFs, generated offline and identical on every OS.
- **Dashboard** — money owed, overdue count, invoice counts, low stock, and recent activity.
- **Backup & restore** — your data is a single SQLite file. RMB backs it up automatically on every
  launch (the latest 7, plus one a day for a month), before every upgrade and before every restore.
  Manual backups go wherever you choose, and a restore is validated and upgraded before it replaces
  anything.

The interface targets WCAG 2.1 AA, has light and dark themes, visible focus states and automated
screen-level accessibility and colour-contrast checks.

## Install

Once a release has passed the checklist, grab the installer for your OS from the Releases page.

**macOS (`.dmg`)** — open it and drag RMB to Applications. Public builds should be signed and
notarised. Local developer builds are unsigned and are not suitable for general distribution.

**Windows (`.exe`)** — run the installer; it installs per-user, no admin needed. SmartScreen may warn
about an unrecognised app on unsigned builds — choose **More info → Run anyway**.

### Where your data lives

A single SQLite file in your user profile:

- **macOS** — `~/Library/Application Support/com.rhysellwood.rmb/rmb.sqlite`
- **Windows** — `%APPDATA%\com.rhysellwood.rmb\rmb.sqlite`

It isn't encrypted at rest — RMB relies on your OS user account and full-disk encryption (FileVault /
BitLocker). Nothing is ever sent anywhere. Automatic backups sit in a `backups` folder alongside it, and
**Settings → Back up** writes a copy wherever you choose.

## Tech

- **Tauri 2** (Rust core) — small, native, fully offline.
- **React 19 + TypeScript + Vite + Tailwind + shadcn/ui** on the front end.
- **SQLite** (bundled) via **sqlx**, with migrations.
- Money is handled as **integer minor units** in a pure Rust domain crate, property-tested so the
  financial maths is exact — no floating-point drift in tax or totals.

The workspace is layered so the rules stay testable and framework-free: `rmb-domain` (pure logic) →
`rmb-data` (sqlx repositories) → `src-tauri` (thin command layer) → React UI.

## Build from source

Requires **Node ≥ 20.19**, **Rust ≥ 1.96**, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri dev      # run in development
npm run tauri build    # build an installer for your OS
```

Checks:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
npm run check          # versions, IPC surface, types, lint, formatting, tests, build
```

There are 223 automated tests: 117 Rust tests and 106 front-end tests. Money, tax and inventory rules
are property-tested. Repositories run against real SQLite databases. Concurrency tests repeat races in
payments, numbering, stock, quote conversion, job billing and recurring generation, on databases
configured exactly as the app configures them. An invariants test drives hundreds of random actions and
checks after each one that stock, document totals, invoice status and every report still agree. Every
screen, detail view, form and key dialog is checked for accessibility violations, and colour contrast
is checked from the stylesheet in both themes. CI runs all of it on every pull request. Automated
tests don't replace a manual pass over the installers and core workflows on clean machines before
each release.

## Roadmap

The core is deliberately general, with room to grow one module at a time. Not built yet:

- Emailing documents, and online card payments (Stripe).
- Credit notes, refunds and customer account credits.
- Scheduling and dispatch.
- Expenses, purchasing, suppliers, bills, bank reconciliation and double-entry accounting.
- A mobile app and multi-user sync — the local-first architecture leaves room for this.
- Code signing and notarisation for distribution.

Issues and pull requests are welcome.

## Licence

**GPL-3.0-or-later** — see [LICENSE](LICENSE). This keeps RMB and anything derived from it free and
open. Dependency licences are checked against the reviewed allow-list in [deny.toml](deny.toml).
