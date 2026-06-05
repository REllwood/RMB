//! Document numbering — formatting a gapless sequence into a display number.
//!
//! The sequence counter lives in settings and is incremented transactionally at *issue* time
//! (so issued documents get gapless numbers; drafts have none). This module only formats.

/// Format a document number from a prefix and zero-padded sequence.
/// e.g. `format_number("INV-", 42, 5)` → `"INV-00042"`; `format_number("Q", 7, 0)` → `"Q7"`.
pub fn format_number(prefix: &str, seq: i64, pad: usize) -> String {
    format!("{prefix}{seq:0pad$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_and_prefixes() {
        assert_eq!(format_number("INV-", 42, 5), "INV-00042");
        assert_eq!(format_number("Q", 7, 0), "Q7");
        assert_eq!(format_number("", 100, 3), "100");
        assert_eq!(format_number("EST-", 12345, 4), "EST-12345");
    }
}
