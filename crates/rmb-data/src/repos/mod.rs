//! Repositories — the only place SQL is executed. Parameterized queries only (never string
//! interpolation). Each module owns one aggregate; pure rules live in `rmb-domain`.

pub mod customers;
pub mod dashboard;
pub mod invoices;
pub mod items;
pub mod jobs;
pub mod meta;
pub mod payments;
pub mod quotes;
pub mod recurring;
pub mod reports;
pub mod settings;
