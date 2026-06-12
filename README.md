# RMB — open-source business manager for small & home businesses

A **free, open-source desktop app** that gives any small business one place to run the core loop —
customers, catalog, stock, quotes, invoices, payments — with **all data on your own machine**. No
subscriptions, no cloud, works fully offline.

> Status: **v1 complete & tested.** Customers, catalog, quotes, jobs/timekeeping, invoices (incl.
> recurring), payments, reports, PDF export, and the dashboard all work end to end — **80 tests**
> including a full golden-path E2E (money/tax/inventory property-tested, every screen
> accessibility-tested). See [Roadmap](#roadmap) for what's next.

## What it does today

- **Business setup** — your details, **logo** (shown on PDFs), currency, and a **configurable tax
  engine** with one-click presets for the UK (VAT), Australia / New Zealand (GST), Canada, and the US,
  plus custom rates.
- **Customers** — searchable records, each with its **full history**: every quote, job, and invoice in
  one place.
- **Catalog & inventory** — products (with tracked stock) and services; an **append-only stock ledger**
  with low-stock alerts. Catalog items are **picked straight onto quotes, jobs, and invoices** (default
  price + tax applied), so selling actually moves stock.
- **Quotes** — estimates with the same exact tax; edit drafts, send → accept, then **convert into an
  invoice or a job** in one click.
- **Jobs & timekeeping** — track time (dated entries, hours × rate) and materials (from the catalog or
  free-form) against a job, then **create an invoice from the job** in one step — billed work can't be
  pulled twice.
- **Invoices** — build from catalog items or free lines, with **exact, per-line tax** (inclusive or
  exclusive, multi-rate), due dates, and **overdue** flags. Drafts are editable/deletable; **issuing**
  assigns a **gapless number**, **freezes a snapshot**, and **decrements stock once**; issued invoices
  are immutable (correct via **void + reissue**).
- **Recurring invoices** — weekly / fortnightly / monthly / quarterly / yearly schedules that
  auto-draft when due (with catch-up after time away); you still review and issue each one.
- **Payments** — record partial/full payments with method + reference; status flows unpaid → part-paid
  → paid with a live balance, a mis-entered payment can be **removed** (status recalculates), and any
  paid invoice can produce a **receipt PDF**.
- **Reports & exports** — tax collected per rate for any period (your BAS / VAT-return numbers), sales
  by month and by customer, plus **CSV exports** (invoices, payments, customers) for your accountant.
- **PDF export** — branded (logo + business details), fully offline invoice, quote & receipt PDFs
  (Typst with embedded fonts — identical on every OS).
- **Dashboard** — money owed, **overdue count**, invoice counts, low stock, recent activity, and a
  first-run setup guide.
- **Backup & restore** — your data is a single SQLite file; an **automatic rotating backup** on every
  launch (last 7 kept), one-click safe manual backup (`VACUUM INTO`), and integrity-checked restore.

Everything is **keyboard-accessible** (WCAG 2.1 AA target) with light + dark themes.

## Tech

Local-first desktop app:

- **Tauri 2** (Rust core) — tiny, native, fully offline.
- **React 19 + TypeScript + Vite 8 + Tailwind v4 + shadcn/ui** frontend.
- **SQLite** (bundled) via **sqlx**, with migrations.
- A pure **`rmb-domain`** Rust crate holds all money/tax/inventory rules — **integer-minor-unit money**,
  property-tested so the financial maths is exact.

Architecture is a Cargo workspace: `rmb-domain` (pure logic) → `rmb-data` (sqlx repositories) →
`src-tauri` (thin commands) → React UI.

## Build from source

Requirements: **Node ≥ 20.19** (CI uses 22), **Rust ≥ 1.96**, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri dev      # run the app in development
npm run tauri build    # build an installer for your OS
```

Quality gates:

```bash
cargo test --workspace                       # Rust unit + integration tests
cargo clippy --workspace --all-targets -- -D warnings
npm run check                                # tsc + eslint + vitest (axe) + build
```

## Data & backups

Your data lives in a single SQLite file in your OS app-data directory
(`~/Library/Application Support/com.rhysellwood.rmb/rmb.sqlite` on macOS). Use **Settings → Back up** to
export a copy, and **Restore** to load one. See [docs/INSTALL.md](docs/INSTALL.md).

## Roadmap

Built on a general core designed to grow Odoo-style, one module at a time. Not yet implemented:

- **Email** sending, **online card payments** (Stripe), scheduling/dispatch.
- **Double-entry accounting**, purchasing / supplier bills.
- **Mobile app + multi-user sync** (the architecture is local-first to allow this later).
- **Code signing** for distribution (the release CI is already wired for it).

## Licence

**GPL-3.0-or-later** — see [LICENSE](LICENSE). Keeps the app and any derivatives free and open. All
dependencies are permissive (MIT/Apache/BSD) and compatible (verified with `cargo deny`).

🤖 Built with [Claude Code](https://claude.com/claude-code) via the `forge` pipeline.
