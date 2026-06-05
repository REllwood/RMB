//! Jobs with time entries + materials. Invoicing a job creates a draft invoice (labour from time,
//! products/services from materials) and marks the source rows invoiced so they can't be billed twice.
//! Jobs never touch stock — that happens once, when the resulting invoice is issued.

use std::str::FromStr;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;
use crate::repos::invoices::{self, LineInput};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Job {
    pub id: i64,
    pub customer_id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub source_quote_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobInput {
    pub customer_id: i64,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TimeEntry {
    pub id: i64,
    pub date: String,
    pub minutes: i64,
    pub rate_minor: i64,
    pub description: String,
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub tax_inclusive: bool,
    pub invoiced: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeEntryInput {
    pub date: String,
    pub minutes: i64,
    pub rate_minor: i64,
    pub description: String,
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub tax_inclusive: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JobMaterial {
    pub id: i64,
    pub item_id: Option<i64>,
    pub description: String,
    pub quantity: String,
    pub unit_price_minor: i64,
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub tax_inclusive: bool,
    pub invoiced: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobMaterialInput {
    pub item_id: Option<i64>,
    pub description: String,
    pub quantity: String,
    pub unit_price_minor: i64,
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub tax_inclusive: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobDetail {
    pub job: Job,
    pub time_entries: Vec<TimeEntry>,
    pub materials: Vec<JobMaterial>,
    pub labour_total_minor: i64,
    pub materials_total_minor: i64,
}

pub async fn create(db: &Db, input: &JobInput) -> Result<i64, DataError> {
    let id = sqlx::query("INSERT INTO job (customer_id, title, description) VALUES (?, ?, ?)")
        .bind(input.customer_id)
        .bind(&input.title)
        .bind(&input.description)
        .execute(db)
        .await?
        .last_insert_rowid();
    Ok(id)
}

pub async fn list(db: &Db) -> Result<Vec<Job>, DataError> {
    Ok(sqlx::query_as::<_, Job>(
        "SELECT id, customer_id, title, description, status, source_quote_id, created_at \
         FROM job WHERE deleted_at IS NULL ORDER BY id DESC",
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_status(db: &Db, id: i64, status: &str) -> Result<(), DataError> {
    sqlx::query("UPDATE job SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete(db: &Db, id: i64) -> Result<(), DataError> {
    sqlx::query("UPDATE job SET deleted_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn add_time(db: &Db, job_id: i64, t: &TimeEntryInput) -> Result<i64, DataError> {
    let id = sqlx::query(
        "INSERT INTO time_entry (job_id, date, minutes, rate_minor, description, tax_rate_name, tax_rate_bp, tax_inclusive) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job_id)
    .bind(&t.date)
    .bind(t.minutes)
    .bind(t.rate_minor)
    .bind(&t.description)
    .bind(&t.tax_rate_name)
    .bind(t.tax_rate_bp)
    .bind(t.tax_inclusive)
    .execute(db)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn add_material(db: &Db, job_id: i64, m: &JobMaterialInput) -> Result<i64, DataError> {
    let id = sqlx::query(
        "INSERT INTO job_material (job_id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, tax_inclusive) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job_id)
    .bind(m.item_id)
    .bind(&m.description)
    .bind(&m.quantity)
    .bind(m.unit_price_minor)
    .bind(&m.tax_rate_name)
    .bind(m.tax_rate_bp)
    .bind(m.tax_inclusive)
    .execute(db)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn delete_time(db: &Db, id: i64) -> Result<(), DataError> {
    sqlx::query("DELETE FROM time_entry WHERE id = ? AND invoiced = 0")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_material(db: &Db, id: i64) -> Result<(), DataError> {
    sqlx::query("DELETE FROM job_material WHERE id = ? AND invoiced = 0")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

fn hours(minutes: i64) -> Decimal {
    (Decimal::from(minutes) / Decimal::from(60)).round_dp(2)
}

pub async fn get_detail(db: &Db, id: i64) -> Result<Option<JobDetail>, DataError> {
    let job = sqlx::query_as::<_, Job>(
        "SELECT id, customer_id, title, description, status, source_quote_id, created_at \
         FROM job WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    let Some(job) = job else { return Ok(None) };

    let time_entries = sqlx::query_as::<_, TimeEntry>(
        "SELECT id, date, minutes, rate_minor, description, tax_rate_name, tax_rate_bp, tax_inclusive, invoiced \
         FROM time_entry WHERE job_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let materials = sqlx::query_as::<_, JobMaterial>(
        "SELECT id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, tax_inclusive, invoiced \
         FROM job_material WHERE job_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(db)
    .await?;

    let labour_total_minor: i64 = time_entries
        .iter()
        .map(|t| {
            (Decimal::from(t.rate_minor) * hours(t.minutes))
                .round()
                .to_i64()
                .unwrap_or(0)
        })
        .sum();
    let materials_total_minor: i64 = materials
        .iter()
        .map(|m| {
            let qty = Decimal::from_str(m.quantity.trim()).unwrap_or(Decimal::ZERO);
            (Decimal::from(m.unit_price_minor) * qty)
                .round()
                .to_i64()
                .unwrap_or(0)
        })
        .sum();

    Ok(Some(JobDetail {
        job,
        time_entries,
        materials,
        labour_total_minor,
        materials_total_minor,
    }))
}

/// Create a draft invoice from the job's **un-invoiced** time + materials, mark them invoiced, and
/// link the job. Returns the new invoice id. Errors if there's nothing un-invoiced.
pub async fn invoice_from_job(db: &Db, id: i64) -> Result<i64, DataError> {
    let detail = get_detail(db, id)
        .await?
        .ok_or_else(|| DataError::Other("job not found".into()))?;

    let mut lines: Vec<LineInput> = Vec::new();
    for t in detail.time_entries.iter().filter(|t| !t.invoiced) {
        let desc = if t.description.trim().is_empty() {
            format!("Labour ({})", t.date)
        } else {
            format!("Labour: {} ({})", t.description, t.date)
        };
        lines.push(LineInput {
            item_id: None,
            description: desc,
            quantity: hours(t.minutes).to_string(),
            unit_price_minor: t.rate_minor,
            tax_rate_name: t.tax_rate_name.clone(),
            tax_rate_bp: t.tax_rate_bp,
            tax_inclusive: t.tax_inclusive,
        });
    }
    for m in detail.materials.iter().filter(|m| !m.invoiced) {
        lines.push(LineInput {
            item_id: m.item_id,
            description: m.description.clone(),
            quantity: m.quantity.clone(),
            unit_price_minor: m.unit_price_minor,
            tax_rate_name: m.tax_rate_name.clone(),
            tax_rate_bp: m.tax_rate_bp,
            tax_inclusive: m.tax_inclusive,
        });
    }
    if lines.is_empty() {
        return Err(DataError::Other(
            "nothing to invoice — add time or materials first".into(),
        ));
    }

    // create_draft manages its own transaction; then mark source rows + link the invoice.
    let invoice_id = invoices::create_draft(db, detail.job.customer_id, &lines, None, "").await?;

    let mut tx = db.begin().await?;
    sqlx::query("UPDATE time_entry SET invoiced = 1 WHERE job_id = ? AND invoiced = 0")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE job_material SET invoiced = 1 WHERE job_id = ? AND invoiced = 0")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE job SET status = 'invoiced' WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE invoice SET source_job_id = ? WHERE id = ?")
        .bind(id)
        .bind(invoice_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(invoice_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::customers;

    #[sqlx::test]
    async fn job_time_materials_and_invoice(pool: Db) -> Result<(), DataError> {
        let customer = customers::create(
            &pool,
            &customers::CustomerInput {
                name: "Acme".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await?;
        let job = create(
            &pool,
            &JobInput {
                customer_id: customer,
                title: "Rewire".into(),
                description: String::new(),
            },
        )
        .await?;

        // 90 minutes @ $60/hr = 1.5h × $60 = $90.00 labour
        add_time(
            &pool,
            job,
            &TimeEntryInput {
                date: "2026-06-05".into(),
                minutes: 90,
                rate_minor: 6000,
                description: "fit consumer unit".into(),
                tax_rate_name: "No Tax".into(),
                tax_rate_bp: 0,
                tax_inclusive: false,
            },
        )
        .await?;
        // 3 × $5.00 cable = $15.00 materials
        add_material(
            &pool,
            job,
            &JobMaterialInput {
                item_id: None,
                description: "Cable".into(),
                quantity: "3".into(),
                unit_price_minor: 500,
                tax_rate_name: "No Tax".into(),
                tax_rate_bp: 0,
                tax_inclusive: false,
            },
        )
        .await?;

        let d = get_detail(&pool, job).await?.unwrap();
        assert_eq!(d.labour_total_minor, 9000);
        assert_eq!(d.materials_total_minor, 1500);

        let invoice_id = invoice_from_job(&pool, job).await?;
        let inv = invoices::get_detail(&pool, invoice_id).await?.unwrap();
        assert_eq!(inv.invoice.total_minor, 10500); // 90 + 15
        assert_eq!(inv.lines.len(), 2);

        // re-invoicing pulls nothing (all marked invoiced)
        assert!(invoice_from_job(&pool, job).await.is_err());
        assert_eq!(
            get_detail(&pool, job).await?.unwrap().job.status,
            "invoiced"
        );
        Ok(())
    }
}
