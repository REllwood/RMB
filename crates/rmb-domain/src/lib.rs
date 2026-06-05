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

/// Walking-skeleton greeting — proves the UI → command → domain round-trip.
/// Replaced/extended by real domain modules in later phases.
pub fn greet(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "Hello there! You've been greeted from RMB's Rust core.".to_string()
    } else {
        format!("Hello, {name}! You've been greeted from RMB's Rust core.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_uses_name() {
        assert_eq!(
            greet("Ada"),
            "Hello, Ada! You've been greeted from RMB's Rust core."
        );
    }

    #[test]
    fn greet_handles_blank() {
        assert!(greet("   ").starts_with("Hello there!"));
    }
}
