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

/// A `LIKE` pattern that matches `search` literally anywhere in the text: `%`, `_` and `\` typed by
/// the user are escaped (queries use `ESCAPE '\'`) rather than acting as wildcards.
pub(crate) fn like_pattern(search: Option<&str>) -> String {
    let escaped = search
        .unwrap_or("")
        .trim()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

#[cfg(test)]
mod tests {
    #[test]
    fn like_patterns_are_literal() {
        assert_eq!(super::like_pattern(Some(" 50% off_ ")), "%50\\% off\\_%");
        assert_eq!(super::like_pattern(None), "%%");
    }
}
