//! Small, shared validation helpers for values that cross the desktop IPC boundary.

/// Strict Gregorian calendar date in the business-data format used by SQLite (`YYYY-MM-DD`).
pub(crate) fn valid_business_date(value: &str) -> bool {
    let mut parts = value.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        year.parse::<i32>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year > 0 && (1..=max_day).contains(&day)
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
}
