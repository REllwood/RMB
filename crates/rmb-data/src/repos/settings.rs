//! Business settings (singleton, id = 1) + configurable tax rates with country presets.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::{begin_write, Db};
use crate::error::DataError;

/// v1 formats and parses all money with two minor-unit decimal places. Restrict settings to
/// currencies with that exponent until variable-exponent currency support is implemented.
const SUPPORTED_CURRENCIES: [&str; 6] = ["AUD", "CAD", "EUR", "GBP", "NZD", "USD"];

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Settings {
    pub business_name: String,
    pub address: String,
    pub email: String,
    pub phone: String,
    pub logo_path: Option<String>,
    pub currency: String,
    pub tax_label: String,
    pub tax_number: String,
    pub prices_tax_inclusive: bool,
    pub invoice_prefix: String,
    pub invoice_next_seq: i64,
    pub quote_prefix: String,
    pub quote_next_seq: i64,
    pub number_pad: i64,
    /// The tax rate new document lines start with (None: the first non-zero rate).
    #[serde(default)]
    pub default_tax_rate_id: Option<i64>,
    /// Payment terms for new drafts without a due date: due this many days after issue.
    #[serde(default)]
    pub default_due_days: Option<i64>,
    /// Read-only: true once an invoice has been issued, after which the currency can't change
    /// (issued invoices and every total are in that currency).
    #[serde(default)]
    pub currency_locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoAsset {
    pub data: Vec<u8>,
    pub format: String,
}

pub async fn get(db: &Db) -> Result<Settings, DataError> {
    Ok(sqlx::query_as::<_, Settings>(
        "SELECT business_name, address, email, phone, logo_path, currency, tax_label, tax_number, \
         prices_tax_inclusive, invoice_prefix, invoice_next_seq, quote_prefix, quote_next_seq, \
         number_pad, default_tax_rate_id, default_due_days, \
         EXISTS(SELECT 1 FROM invoice WHERE status <> 'draft') AS currency_locked \
         FROM settings WHERE id = 1",
    )
    .fetch_one(db)
    .await?)
}

fn validate_text(label: &str, value: &str, maximum: usize) -> Result<String, DataError> {
    let value = value.trim().to_owned();
    if value.chars().count() > maximum {
        return Err(DataError::Other(format!(
            "{label} cannot exceed {maximum} characters"
        )));
    }
    Ok(value)
}

/// Update the editable settings fields (sequence counters are managed by issue logic, not here).
pub async fn update(db: &Db, s: &Settings) -> Result<(), DataError> {
    let business_name = validate_text("business name", &s.business_name, 200)?;
    let address = validate_text("business address", &s.address, 2_000)?;
    let email = validate_text("business email", &s.email, 320)?;
    let phone = validate_text("business phone", &s.phone, 100)?;
    let currency = s.currency.trim().to_ascii_uppercase();
    if !SUPPORTED_CURRENCIES.contains(&currency.as_str()) {
        return Err(DataError::Other(format!(
            "currency must be one of {}",
            SUPPORTED_CURRENCIES.join(", ")
        )));
    }
    let tax_label = validate_text("tax label", &s.tax_label, 50)?;
    if tax_label.is_empty() {
        return Err(DataError::Other("tax label is required".into()));
    }
    let tax_number = validate_text("tax number", &s.tax_number, 100)?;
    let invoice_prefix = validate_text("invoice prefix", &s.invoice_prefix, 20)?;
    let quote_prefix = validate_text("quote prefix", &s.quote_prefix, 20)?;
    if invoice_prefix.is_empty() || quote_prefix.is_empty() {
        return Err(DataError::Other(
            "invoice and quote prefixes are required".into(),
        ));
    }
    if !(1..=12).contains(&s.number_pad) {
        return Err(DataError::Other(
            "number padding must be between 1 and 12 digits".into(),
        ));
    }
    if s.default_due_days
        .is_some_and(|days| !(0..=3_650).contains(&days))
    {
        return Err(DataError::Other(
            "default payment terms must be between 0 and 3,650 days".into(),
        ));
    }
    if let Some(tax_id) = s.default_tax_rate_id {
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tax_rate WHERE id = ? AND archived = 0)",
        )
        .bind(tax_id)
        .fetch_one(db)
        .await?;
        if !active {
            return Err(DataError::Other(
                "the default tax rate was not found".into(),
            ));
        }
    }
    let (stored_currency, locked): (String, bool) = sqlx::query_as(
        "SELECT currency, EXISTS(SELECT 1 FROM invoice WHERE status <> 'draft') \
         FROM settings WHERE id = 1",
    )
    .fetch_one(db)
    .await?;
    if locked && stored_currency != currency {
        return Err(DataError::Other(format!(
            "the currency can't change after invoices have been issued in {stored_currency}; \
             totals and reports would mix currencies"
        )));
    }

    let result = sqlx::query(
        "UPDATE settings SET business_name=?, address=?, email=?, phone=?, currency=?, \
         tax_label=?, tax_number=?, prices_tax_inclusive=?, invoice_prefix=?, quote_prefix=?, \
         number_pad=?, default_tax_rate_id=?, default_due_days=? WHERE id = 1",
    )
    .bind(business_name)
    .bind(address)
    .bind(email)
    .bind(phone)
    .bind(currency)
    .bind(tax_label)
    .bind(tax_number)
    .bind(s.prices_tax_inclusive)
    .bind(invoice_prefix)
    .bind(quote_prefix)
    .bind(s.number_pad)
    .bind(s.default_tax_rate_id)
    .bind(s.default_due_days)
    .execute(db)
    .await?;
    if result.rows_affected() != 1 {
        return Err(DataError::Other("business settings were not found".into()));
    }
    Ok(())
}

/// Return the embedded current logo. The database copy makes a `.sqlite` backup self-contained.
pub async fn get_logo_asset(db: &Db) -> Result<Option<LogoAsset>, DataError> {
    let (data, format): (Option<Vec<u8>>, Option<String>) = sqlx::query_as(
        "SELECT a.data, a.format FROM settings s \
             LEFT JOIN business_logo_asset a ON a.id = s.logo_asset_id WHERE s.id = 1",
    )
    .fetch_one(db)
    .await?;
    match (data, format) {
        (None, None) => Ok(None),
        (Some(data), Some(format))
            if !data.is_empty() && matches!(format.as_str(), "png" | "jpg") =>
        {
            Ok(Some(LogoAsset { data, format }))
        }
        _ => Err(DataError::Other(
            "stored business logo is incomplete or has an invalid format".into(),
        )),
    }
}

/// Logo assets are controlled by the native import command, not accepted from frontend settings.
pub async fn set_logo_asset(
    db: &Db,
    path: Option<&str>,
    asset: Option<(&[u8], &str)>,
) -> Result<(), DataError> {
    let mut tx = begin_write(db).await?;
    let asset_id = match asset {
        Some((data, format)) if !data.is_empty() && matches!(format, "png" | "jpg") => {
            let existing: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM business_logo_asset WHERE format = ? AND data = ? LIMIT 1",
            )
            .bind(format)
            .bind(data)
            .fetch_optional(&mut *tx)
            .await?;
            Some(match existing {
                Some(id) => id,
                None => sqlx::query("INSERT INTO business_logo_asset (format, data) VALUES (?, ?)")
                    .bind(format)
                    .bind(data)
                    .execute(&mut *tx)
                    .await?
                    .last_insert_rowid(),
            })
        }
        Some(_) => {
            return Err(DataError::Other(
                "logo asset must be a non-empty PNG or JPEG".into(),
            ))
        }
        None => None,
    };
    let result = sqlx::query("UPDATE settings SET logo_path = ?, logo_asset_id = ? WHERE id = 1")
        .bind(path)
        .bind(asset_id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() != 1 {
        return Err(DataError::Other("business settings were not found".into()));
    }
    tx.commit().await?;
    Ok(())
}

/// Attach the newly embedded current asset to invoices issued by path-only pre-0011 builds when
/// their frozen path matches. This is best-effort historical recovery and is safe to rerun.
pub async fn backfill_legacy_invoice_logo_assets(db: &Db) -> Result<u64, DataError> {
    let updated = sqlx::query(
        "UPDATE invoice SET business_logo_asset_id = (SELECT logo_asset_id FROM settings WHERE id = 1) \
         WHERE business_logo_asset_id IS NULL AND business_snapshot IS NOT NULL \
           AND (SELECT logo_asset_id FROM settings WHERE id = 1) IS NOT NULL \
           AND CASE WHEN json_valid(business_snapshot) \
                    THEN json_extract(business_snapshot, '$.logo_path') END \
               = (SELECT logo_path FROM settings WHERE id = 1)",
    )
    .execute(db)
    .await?
    .rows_affected();
    Ok(updated)
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaxRateRow {
    pub id: i64,
    pub name: String,
    pub rate_bp: i64,
    pub inclusive: bool,
    pub archived: bool,
}

pub async fn list_tax_rates(db: &Db, include_archived: bool) -> Result<Vec<TaxRateRow>, DataError> {
    let sql = if include_archived {
        "SELECT id, name, rate_bp, inclusive, archived FROM tax_rate ORDER BY archived, name"
    } else {
        "SELECT id, name, rate_bp, inclusive, archived FROM tax_rate WHERE archived = 0 ORDER BY name"
    };
    Ok(sqlx::query_as::<_, TaxRateRow>(sql).fetch_all(db).await?)
}

pub async fn create_tax_rate(
    db: &Db,
    name: &str,
    rate_bp: i64,
    inclusive: bool,
) -> Result<i64, DataError> {
    let name = validate_tax_rate(db, None, name, rate_bp).await?;
    let id = sqlx::query("INSERT INTO tax_rate (name, rate_bp, inclusive) VALUES (?, ?, ?)")
        .bind(name)
        .bind(rate_bp)
        .bind(inclusive)
        .execute(db)
        .await?
        .last_insert_rowid();
    Ok(id)
}

pub async fn update_tax_rate(
    db: &Db,
    id: i64,
    name: &str,
    rate_bp: i64,
    inclusive: bool,
) -> Result<(), DataError> {
    let name = validate_tax_rate(db, Some(id), name, rate_bp).await?;
    let result = sqlx::query(
        "UPDATE tax_rate SET name=?, rate_bp=?, inclusive=? WHERE id=? AND archived = 0",
    )
    .bind(name)
    .bind(rate_bp)
    .bind(inclusive)
    .bind(id)
    .execute(db)
    .await?;
    if result.rows_affected() != 1 {
        return Err(DataError::Other("tax rate not found".into()));
    }
    Ok(())
}

/// Make an archived tax rate available again (its name must not clash with an active rate).
pub async fn restore_tax_rate(db: &Db, id: i64) -> Result<(), DataError> {
    let name: Option<String> =
        sqlx::query_scalar("SELECT name FROM tax_rate WHERE id = ? AND archived = 1")
            .bind(id)
            .fetch_optional(db)
            .await?;
    let name = name.ok_or_else(|| DataError::Other("archived tax rate not found".into()))?;
    let clash: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tax_rate WHERE archived = 0 AND lower(name) = lower(?))",
    )
    .bind(&name)
    .fetch_one(db)
    .await?;
    if clash {
        return Err(DataError::Other(format!(
            "an active tax rate is already called {name}; rename or archive it first"
        )));
    }
    sqlx::query("UPDATE tax_rate SET archived = 0 WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn archive_tax_rate(db: &Db, id: i64) -> Result<(), DataError> {
    let item_uses: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM item WHERE default_tax_rate_id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    if item_uses > 0 {
        return Err(DataError::Other(
            "tax rate cannot be archived while active catalogue items use it".into(),
        ));
    }
    let result = sqlx::query("UPDATE tax_rate SET archived = 1 WHERE id = ? AND archived = 0")
        .bind(id)
        .execute(db)
        .await?;
    sqlx::query("UPDATE settings SET default_tax_rate_id = NULL WHERE default_tax_rate_id = ?")
        .bind(id)
        .execute(db)
        .await?;
    if result.rows_affected() != 1 {
        return Err(DataError::Other("tax rate not found".into()));
    }
    Ok(())
}

async fn validate_tax_rate(
    db: &Db,
    exclude_id: Option<i64>,
    name: &str,
    rate_bp: i64,
) -> Result<String, DataError> {
    let name = validate_text("tax-rate name", name, 100)?;
    if name.is_empty() {
        return Err(DataError::Other("tax-rate name is required".into()));
    }
    if !(0..=100_000).contains(&rate_bp) {
        return Err(DataError::Other(
            "tax rate must be between 0% and 1,000%".into(),
        ));
    }
    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tax_rate WHERE archived = 0 \
         AND lower(name) = lower(?) AND id != ?)",
    )
    .bind(&name)
    .bind(exclude_id.unwrap_or(-1))
    .fetch_one(db)
    .await?;
    if duplicate {
        return Err(DataError::Other(
            "an active tax rate already has this name".into(),
        ));
    }
    Ok(name)
}

/// `(name, rate_bp)` tax presets per country (editable after seeding).
pub fn tax_preset(country: &str) -> Option<Vec<(&'static str, i64)>> {
    match country {
        "UK" => Some(vec![
            ("VAT 20%", 2000),
            ("VAT 5%", 500),
            ("VAT 0%", 0),
            ("No Tax", 0),
        ]),
        "AU" => Some(vec![("GST 10%", 1000), ("GST Free", 0)]),
        "NZ" => Some(vec![("GST 15%", 1500), ("Zero-rated", 0)]),
        "CA" => Some(vec![
            ("GST 5%", 500),
            ("HST 13% (Ontario)", 1300),
            ("HST 14% (Nova Scotia)", 1400),
            ("HST 15%", 1500),
            ("Zero-rated", 0),
            ("No Tax", 0),
        ]),
        "US" | "OTHER" => Some(vec![("No Tax", 0)]),
        _ => None,
    }
}

pub async fn apply_tax_preset(db: &Db, country: &str) -> Result<(), DataError> {
    let country = country.trim().to_ascii_uppercase();
    let preset =
        tax_preset(&country).ok_or_else(|| DataError::Other("unknown tax preset".into()))?;
    let mut tx = begin_write(db).await?;
    let inclusive_default: bool =
        sqlx::query_scalar("SELECT prices_tax_inclusive FROM settings WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    let mut first_rate = None;
    for (name, rate_bp) in preset {
        let existing: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM tax_rate WHERE archived = 0 AND lower(name) = lower(?)",
        )
        .bind(name)
        .fetch_optional(&mut *tx)
        .await?;
        let id = match existing {
            Some(id) => id,
            None => sqlx::query("INSERT INTO tax_rate (name, rate_bp, inclusive) VALUES (?, ?, ?)")
                .bind(name)
                .bind(rate_bp)
                .bind(rate_bp > 0 && inclusive_default)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid(),
        };
        first_rate.get_or_insert(id);
    }
    // Each preset lists its standard rate first; make it the default unless one is already chosen.
    sqlx::query(
        "UPDATE settings SET default_tax_rate_id = ? WHERE id = 1 AND default_tax_rate_id IS NULL",
    )
    .bind(first_rate)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::customers;

    #[sqlx::test]
    async fn settings_roundtrip(pool: Db) -> Result<(), DataError> {
        let mut s = get(&pool).await?;
        assert_eq!(s.currency, "USD");
        s.business_name = "Acme Plumbing".into();
        s.currency = "GBP".into();
        s.prices_tax_inclusive = true;
        update(&pool, &s).await?;

        let s2 = get(&pool).await?;
        assert_eq!(s2.business_name, "Acme Plumbing");
        assert_eq!(s2.currency, "GBP");
        assert!(s2.prices_tax_inclusive);

        set_logo_asset(&pool, Some("/test/logo.png"), Some((b"png-bytes", "png"))).await?;
        assert_eq!(
            get_logo_asset(&pool).await?,
            Some(LogoAsset {
                data: b"png-bytes".to_vec(),
                format: "png".into(),
            })
        );
        set_logo_asset(
            &pool,
            Some("/test/same-logo-new-path.png"),
            Some((b"png-bytes", "png")),
        )
        .await?;
        let asset_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM business_logo_asset")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            asset_count, 1,
            "identical logo imports should be deduplicated"
        );

        let customer = customers::create(
            &pool,
            &customers::CustomerInput {
                name: "Legacy customer".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await?;
        sqlx::query(
            "INSERT INTO invoice (customer_id, status, business_snapshot) \
             VALUES (?, 'issued', '{\"logo_path\":\"/test/same-logo-new-path.png\"}')",
        )
        .bind(customer)
        .execute(&pool)
        .await?;
        assert_eq!(backfill_legacy_invoice_logo_assets(&pool).await?, 1);
        let attached: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM invoice WHERE business_logo_asset_id IS NOT NULL",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(attached, 1);

        set_logo_asset(&pool, None, None).await?;
        assert_eq!(get_logo_asset(&pool).await?, None);
        Ok(())
    }

    #[sqlx::test]
    async fn presets_and_tax_crud(pool: Db) -> Result<(), DataError> {
        apply_tax_preset(&pool, "UK").await?;
        apply_tax_preset(&pool, "UK").await?;
        let rates = list_tax_rates(&pool, false).await?;
        assert_eq!(rates.len(), 4, "reapplying a preset must be idempotent");
        assert!(rates
            .iter()
            .any(|r| r.name == "VAT 20%" && r.rate_bp == 2000));

        let id = create_tax_rate(&pool, "Custom 7.5%", 750, false).await?;
        update_tax_rate(&pool, id, "Custom 7.5%", 750, true).await?;
        archive_tax_rate(&pool, id).await?;

        let active = list_tax_rates(&pool, false).await?;
        assert!(!active.iter().any(|r| r.id == id));
        assert!(update_tax_rate(&pool, id, "Archived", 100, false)
            .await
            .is_err());
        assert!(create_tax_rate(&pool, "  ", 100, false).await.is_err());
        assert!(create_tax_rate(&pool, "Too high", 100_001, false)
            .await
            .is_err());
        assert!(apply_tax_preset(&pool, "not-a-country").await.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn preset_uses_price_mode_and_has_current_canadian_hst_rates(
        pool: Db,
    ) -> Result<(), DataError> {
        let mut settings = get(&pool).await?;
        settings.prices_tax_inclusive = true;
        update(&pool, &settings).await?;
        apply_tax_preset(&pool, "CA").await?;

        let rates = list_tax_rates(&pool, false).await?;
        for basis_points in [500, 1300, 1400, 1500] {
            assert!(rates
                .iter()
                .any(|rate| rate.rate_bp == basis_points && rate.inclusive));
        }
        assert!(rates
            .iter()
            .filter(|rate| rate.rate_bp == 0)
            .all(|rate| !rate.inclusive));
        Ok(())
    }

    #[sqlx::test]
    async fn validates_settings(pool: Db) -> Result<(), DataError> {
        let mut settings = get(&pool).await?;
        settings.currency = "dollars".into();
        assert!(update(&pool, &settings).await.is_err());
        settings.currency = "aud".into();
        settings.number_pad = 0;
        assert!(update(&pool, &settings).await.is_err());
        settings.number_pad = 4;
        update(&pool, &settings).await?;
        assert_eq!(get(&pool).await?.currency, "AUD");
        Ok(())
    }

    #[sqlx::test]
    async fn presets_set_the_default_rate_and_issued_invoices_lock_the_currency(
        pool: Db,
    ) -> Result<(), DataError> {
        apply_tax_preset(&pool, "UK").await?;
        let rates = list_tax_rates(&pool, false).await?;
        let vat = rates.iter().find(|r| r.name == "VAT 20%").unwrap().id;
        assert_eq!(get(&pool).await?.default_tax_rate_id, Some(vat));

        // Re-applying a preset keeps an existing choice.
        let mut s = get(&pool).await?;
        let reduced = rates.iter().find(|r| r.name == "VAT 5%").unwrap().id;
        s.default_tax_rate_id = Some(reduced);
        s.business_name = "Shop".into();
        s.invoice_prefix = "  INV- ".into();
        update(&pool, &s).await?;
        apply_tax_preset(&pool, "UK").await?;
        let saved = get(&pool).await?;
        assert_eq!(saved.default_tax_rate_id, Some(reduced));
        assert_eq!(saved.invoice_prefix, "INV-", "prefixes are stored trimmed");

        // Archiving the default clears it rather than leaving a dangling choice.
        archive_tax_rate(&pool, reduced).await?;
        assert_eq!(get(&pool).await?.default_tax_rate_id, None);

        // Currency is free to change until an invoice is issued.
        let mut s = get(&pool).await?;
        assert!(!s.currency_locked);
        s.currency = "EUR".into();
        update(&pool, &s).await?;
        let customer = customers::create(
            &pool,
            &customers::CustomerInput {
                name: "C".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await?;
        sqlx::query("INSERT INTO invoice (customer_id, status) VALUES (?, 'issued')")
            .bind(customer)
            .execute(&pool)
            .await?;
        let mut s = get(&pool).await?;
        assert!(s.currency_locked);
        s.currency = "GBP".into();
        assert!(update(&pool, &s).await.is_err());
        s.currency = "EUR".into();
        s.phone = "555".into();
        update(&pool, &s).await?;
        Ok(())
    }

    #[sqlx::test]
    async fn archived_tax_rates_can_be_restored(pool: Db) -> Result<(), DataError> {
        let id = create_tax_rate(&pool, "GST 10%", 1000, false).await?;
        archive_tax_rate(&pool, id).await?;
        let replacement = create_tax_rate(&pool, "GST 10%", 1000, true).await?;
        assert!(
            restore_tax_rate(&pool, id).await.is_err(),
            "names stay unique"
        );
        archive_tax_rate(&pool, replacement).await?;
        restore_tax_rate(&pool, id).await?;
        assert!(list_tax_rates(&pool, false)
            .await?
            .iter()
            .any(|r| r.id == id));
        Ok(())
    }
}
