//! Error type for the data layer. Repositories return [`DataError`]; the Tauri layer maps it to
//! its own `AppError` for the frontend using [`DataError::user_message`].

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("{0}")]
    Other(String),
}

impl DataError {
    /// A sentence a person can act on. Validation messages pass through; low-level database
    /// errors (constraint names, SQLite codes) are translated.
    pub fn user_message(&self) -> String {
        match self {
            DataError::Other(message) => message.clone(),
            DataError::Migrate(error) => {
                format!("the database could not be upgraded for this version of RMB ({error})")
            }
            DataError::Sqlx(sqlx::Error::PoolClosed) => {
                "RMB is restarting; try again in a moment".into()
            }
            DataError::Sqlx(sqlx::Error::PoolTimedOut) => {
                "RMB is busy finishing another change; try again".into()
            }
            DataError::Sqlx(sqlx::Error::Database(db)) => {
                let message = db.message();
                if db.is_unique_violation() {
                    return unique_message(message).into();
                }
                if db.is_foreign_key_violation() {
                    return "that record is still used elsewhere, or no longer exists".into();
                }
                match db.code().as_deref() {
                    Some("5" | "517" | "261" | "6") => {
                        "RMB is busy finishing another change; try again".into()
                    }
                    Some("13") => "the disk is full, so the change could not be saved".into(),
                    Some("26") => "the file is not an RMB database".into(),
                    Some("11") => "the database file is damaged; restore a backup".into(),
                    _ => format!("the database reported a problem: {message}"),
                }
            }
            DataError::Sqlx(other) => format!("the database reported a problem: {other}"),
        }
    }
}

/// Which uniqueness rule a change broke, in the user's terms.
fn unique_message(message: &str) -> &'static str {
    if message.contains("idx_item_active_sku_unique") {
        "another catalogue item already uses that SKU"
    } else if message.contains("idx_tax_rate_active_name_unique") {
        "an active tax rate already has that name"
    } else if message.contains("invoice.number") || message.contains("idx_invoice_number_unique") {
        "that invoice number is already used; change the invoice prefix in Settings"
    } else if message.contains("quote.number") || message.contains("idx_quote_number_unique") {
        "that quote number is already used; change the quote prefix in Settings"
    } else if message.contains("source_quote") || message.contains("source_job") {
        "that quote or job has already been converted"
    } else {
        "that would create a duplicate of an existing record"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_messages_pass_through() {
        assert_eq!(
            DataError::Other("customer name is required".into()).user_message(),
            "customer name is required"
        );
        assert_eq!(
            DataError::Sqlx(sqlx::Error::PoolClosed).user_message(),
            "RMB is restarting; try again in a moment"
        );
    }

    #[test]
    fn unique_violations_name_the_rule() {
        assert_eq!(
            unique_message("UNIQUE constraint failed: index 'idx_item_active_sku_unique'"),
            "another catalogue item already uses that SKU"
        );
        assert!(unique_message("UNIQUE constraint failed: invoice.number").contains("prefix"));
    }
}
