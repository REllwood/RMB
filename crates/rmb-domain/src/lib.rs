//! `rmb-domain` — pure business logic for RMB.
//!
//! This crate has **no** dependency on Tauri or sqlx and performs **no** I/O, so every
//! rule here (money, tax, documents, inventory) is unit- and property-testable in isolation.
//! Modules are added as the build progresses (money, tax, totals, status, numbering, inventory).

pub mod document;
pub mod inventory;
pub mod money;
pub mod numbering;
pub mod status;
pub mod tax;

pub use document::{total_lines, DocumentLine};
pub use inventory::{is_low_stock, on_hand, MovementReason};
pub use money::Money;
pub use numbering::format_number;
pub use status::{outstanding, payment_status, InvoiceStatus, QuoteStatus};
pub use tax::{compute_document, line_tax, DocumentTotals, LineTax, RateSummary, TaxLine, TaxRate};
