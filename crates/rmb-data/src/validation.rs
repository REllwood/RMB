//! Small, shared validation helpers for values that cross the desktop IPC boundary.

/// Split a strict Gregorian `YYYY-MM-DD` date (the business-data format SQLite understands) into
/// its parts. Every part must be plain ASCII digits: integer parsing alone would also accept a sign
/// ("2026-+1-15"), which SQLite's date functions reject.
pub(crate) fn date_parts(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    if ![year, month, day]
        .iter()
        .all(|part| part.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let (year, month, day): (i32, u32, u32) =
        (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (year > 0 && (1..=max_day).contains(&day)).then_some((year, month, day))
}

/// Strict Gregorian calendar date in the business-data format used by SQLite (`YYYY-MM-DD`).
pub(crate) fn valid_business_date(value: &str) -> bool {
    date_parts(value).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_calendar_dates() {
        assert!(valid_business_date("2024-02-29"));
        assert!(valid_business_date("2026-12-31"));
        assert!(!valid_business_date("2026-02-29"));
        assert!(!valid_business_date("2026-13-01"));
        assert!(!valid_business_date("2026-1-01"));
        assert!(!valid_business_date("not-a-date"));
    }

    #[test]
    fn rejects_signed_parts() {
        assert!(!valid_business_date("2026-+1-15"));
        assert!(!valid_business_date("+202-01-01"));
        assert!(!valid_business_date("2026-01-+1"));
        assert_eq!(date_parts("2026-01-31"), Some((2026, 1, 31)));
    }
}
