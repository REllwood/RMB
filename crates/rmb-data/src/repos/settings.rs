//! Business settings (singleton, id = 1) + configurable tax rates with country presets.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;

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
}

pub async fn get(db: &Db) -> Result<Settings, DataError> {
    Ok(sqlx::query_as::<_, Settings>(
        "SELECT business_name, address, email, phone, logo_path, currency, tax_label, tax_number, \
         prices_tax_inclusive, invoice_prefix, invoice_next_seq, quote_prefix, quote_next_seq, \
         number_pad FROM settings WHERE id = 1",
    )
    .fetch_one(db)
    .await?)
}

/// Update the editable settings fields (sequence counters are managed by issue logic, not here).
pub async fn update(db: &Db, s: &Settings) -> Result<(), DataError> {
    sqlx::query(
        "UPDATE settings SET business_name=?, address=?, email=?, phone=?, logo_path=?, currency=?, \
         tax_label=?, tax_number=?, prices_tax_inclusive=?, invoice_prefix=?, quote_prefix=?, \
         number_pad=? WHERE id = 1",
    )
    .bind(&s.business_name)
    .bind(&s.address)
    .bind(&s.email)
    .bind(&s.phone)
    .bind(&s.logo_path)
    .bind(&s.currency)
    .bind(&s.tax_label)
    .bind(&s.tax_number)
    .bind(s.prices_tax_inclusive)
    .bind(&s.invoice_prefix)
    .bind(&s.quote_prefix)
    .bind(s.number_pad)
    .execute(db)
    .await?;
    Ok(())
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
    sqlx::query("UPDATE tax_rate SET name=?, rate_bp=?, inclusive=? WHERE id=?")
        .bind(name)
        .bind(rate_bp)
        .bind(inclusive)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn archive_tax_rate(db: &Db, id: i64) -> Result<(), DataError> {
    sqlx::query("UPDATE tax_rate SET archived = 1 WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// `(name, rate_bp, inclusive)` tax presets per country (editable after seeding).
pub fn tax_preset(country: &str) -> Vec<(&'static str, i64, bool)> {
    match country {
        "UK" => vec![
            ("VAT 20%", 2000, false),
            ("VAT 5%", 500, false),
            ("VAT 0%", 0, false),
            ("No Tax", 0, false),
        ],
        "AU" => vec![("GST 10%", 1000, false), ("GST Free", 0, false)],
        "NZ" => vec![("GST 15%", 1500, false), ("Zero-rated", 0, false)],
        "CA" => vec![("GST 5%", 500, false), ("No Tax", 0, false)],
        _ => vec![("No Tax", 0, false)], // US (define your own) + any other country
    }
}

pub async fn apply_tax_preset(db: &Db, country: &str) -> Result<(), DataError> {
    for (name, rate_bp, inclusive) in tax_preset(country) {
        create_tax_rate(db, name, rate_bp, inclusive).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        Ok(())
    }

    #[sqlx::test]
    async fn presets_and_tax_crud(pool: Db) -> Result<(), DataError> {
        apply_tax_preset(&pool, "UK").await?;
        let rates = list_tax_rates(&pool, false).await?;
        assert!(rates
            .iter()
            .any(|r| r.name == "VAT 20%" && r.rate_bp == 2000));

        let id = create_tax_rate(&pool, "Custom 7.5%", 750, false).await?;
        update_tax_rate(&pool, id, "Custom 7.5%", 750, true).await?;
        archive_tax_rate(&pool, id).await?;

        let active = list_tax_rates(&pool, false).await?;
        assert!(!active.iter().any(|r| r.id == id));
        Ok(())
    }
}
