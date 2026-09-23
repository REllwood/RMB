//! `rmb-data` — persistence layer for RMB.
//!
//! Owns the SQLite connection pool, migrations, and one repository module per aggregate
//! (settings, customers, items/stock, quotes, jobs, invoices, payments). Repositories use
//! parameterized `sqlx` queries only; pure rules live in [`rmb_domain`].

pub mod backup;
pub mod db;
pub mod error;
pub mod repos;
mod validation;

pub use error::DataError;

// Re-export the domain crate so the Tauri layer can reach both through `rmb_data`.
pub use rmb_domain;
