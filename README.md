# RMB — business manager for small & home businesses

A free, open-source desktop app that gives a small business one place to run the core loop —
customers, catalog, stock, quotes, invoices, payments — with **all data on your own machine**.
No subscriptions, no cloud, works fully offline.

Most small businesses end up paying £20–50 a month, per user, for a patchwork of tools that each do
one part of this. RMB is an attempt to cover the whole loop in a single app you download once and own.

> **Status: v1 — feature complete and tested.** Everything below works end to end. Windows and macOS
> builds are produced from source today; signed installers are still to come (see [Roadmap](#roadmap)).

## What it does

- **Business setup** — your details, logo (shown on PDFs), currency, and a configurable tax engine
  with one-click presets for the UK (VAT), Australia / New Zealand (GST), Canada, and the US, plus
  custom rates.
- **Customers** — searchable records, each with a full history: every quote, job, and invoice in one
  place.
- **Catalog & inventory** — products (with tracked stock) and services, backed by an append-only stock
  ledger with low-stock alerts. Catalog items are picked straight onto quotes, jobs, and invoices with
  their default price and tax, so selling actually moves stock.
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
  and any paid invoice can produce a receipt PDF.
- **Reports & exports** — tax collected per rate for any period (the numbers your BAS or VAT return
  needs), sales by month and by customer, plus CSV exports of invoices, payments, and customers for
  your accountant.
- **PDF export** — branded invoice, quote, and receipt PDFs, generated offline and identical on every OS.
- **Dashboard** — money owed, overdue count, invoice counts, low stock, and recent activity.
- **Backup & restore** — your data is a single SQLite file. RMB writes a rotating automatic backup on
  every launch (last 7 kept), plus one-click manual backup and an integrity-checked restore.

Everything is keyboard-accessible (WCAG 2.1 AA target) with light and dark themes.

## Install

Grab the installer for your OS from the Releases page.

**macOS (`.dmg`)** — open it and drag RMB to Applications. Builds aren't code-signed yet, so on first
launch macOS may say it "cannot be opened because Apple cannot check it for malicious software". Go to
**System Settings → Privacy & Security**, find the message about RMB, and click **Open Anyway**.

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
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npm run check                                # types, lint, tests, build
```

There are 80 automated tests: the money, tax, and inventory rules are property-tested, every repository
has integration coverage against a real database (including an end-to-end run through the whole
quote → job → invoice → payment flow), and every screen is checked for accessibility violations.

## Roadmap

The core is deliberately general, with room to grow one module at a time. Not built yet:

- Emailing documents, and online card payments (Stripe).
- Scheduling and dispatch.
- Double-entry accounting, purchasing and supplier bills.
- A mobile app and multi-user sync — the local-first architecture leaves room for this.
- Code signing and notarisation for distribution.

Issues and pull requests are welcome.

## Licence

**GPL-3.0-or-later** — see [LICENSE](LICENSE). This keeps RMB and anything derived from it free and
open. All dependencies are permissively licensed (MIT / Apache / BSD) and compatible.
