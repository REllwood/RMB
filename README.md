# RMB — open-source business manager for small & home businesses

A **free, open-source desktop app** that gives any small business one place to run the core loop —
customers, catalog, stock, quotes, invoices, payments — with **all data on your own machine**. No
subscriptions, no cloud, works fully offline.

> Status: **v1 complete & tested.** Customers, catalog, quotes, jobs/timekeeping, invoices, payments,
> PDF export, and the dashboard all work end to end — **66 tests** (money/tax/inventory property-tested,
> every screen accessibility-tested). See [Roadmap](#roadmap) for what's next.

## What it does today

- **Business setup** — your details, currency, and a **configurable tax engine** with one-click presets
  for the UK (VAT), Australia / New Zealand (GST), Canada, and the US, plus custom rates.
- **Customers** — searchable records.
- **Catalog & inventory** — products (with tracked stock) and services; an **append-only stock ledger**
  with low-stock alerts.
- **Quotes** — estimates with the same exact tax; send → accept; **convert a quote into an invoice** in
  one click.
- **Jobs & timekeeping** — track time (hours × rate) and materials against a job, then **create an invoice
  from the job** (labour + materials) in one step.
- **Invoices** — build from line items, with **exact, per-line tax** (inclusive or exclusive, multi-rate).
  Issuing an invoice assigns a **gapless number**, **freezes a snapshot**, and **decrements stock once**;
  issued invoices are immutable (correct via **void + reissue**).
- **Payments** — record partial/full payments; status flows unpaid → part-paid → paid with a live balance.
- **PDF export** — branded, offline invoice & quote PDFs (Typst with embedded fonts — identical on every OS).
- **Dashboard** — money owed, invoice counts, low stock, recent activity.
- **Backup & restore** — your data is a single SQLite file; one-click safe backup (`VACUUM INTO`) and
  integrity-checked restore.

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
- **Double-entry accounting**, purchasing / supplier bills, recurring invoices.
- **Mobile app + multi-user sync** (the architecture is local-first to allow this later).
- **Code signing** for distribution (the release CI is already wired for it).

## Licence

Intended licence: **GPL-3.0-or-later** (keeps the app and any derivatives free and open). All current
dependencies are permissive (MIT/Apache/BSD) and compatible. The final `LICENSE` file is added at first
public release.

🤖 Built with [Claude Code](https://claude.com/claude-code) via the `forge` pipeline.
