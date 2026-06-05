//! `rmb-domain` — pure business logic for RMB.
//!
//! This crate has **no** dependency on Tauri or sqlx and performs **no** I/O, so every
//! rule here (money, tax, documents, inventory) is unit- and property-testable in isolation.
//! Modules are added as the build progresses (money, tax, totals, status, numbering, inventory).

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
