//! `rmb-data` — persistence layer for RMB.
//!
//! Owns the SQLite connection pool, migrations, and one repository module per aggregate
//! (settings, customers, items/stock, quotes, jobs, invoices, payments). Repositories use
//! parameterized `sqlx` queries only; pure rules live in [`rmb_domain`]. Wired up in Phase B.

// Re-export the domain crate so the Tauri layer can reach both through `rmb_data`.
pub use rmb_domain;
