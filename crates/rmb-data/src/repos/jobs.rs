//! Jobs with time entries + materials. Invoicing a job creates a draft invoice (labour from time,
//! products/services from materials) and marks the source rows invoiced so they can't be billed twice.
//! Jobs never touch stock — that happens once, when the resulting invoice is issued.

use std::str::FromStr;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection};

use rmb_domain::status::JobStatus;
use rmb_domain::tax::{line_tax, TaxRate};
use rmb_domain::Money;

use crate::db::{begin_write, Db};
use crate::error::DataError;
use crate::repos::invoices::{self, LineInput};
use crate::validation::valid_business_date;

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
    /// Sum of labour line amounts (as entered: net for exclusive tax, gross for inclusive).
    pub labour_total_minor: i64,
    /// Sum of material line amounts (as entered).
    pub materials_total_minor: i64,
    /// The whole job with tax applied per line, exactly as its invoice would be.
    pub subtotal_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    /// Total (with tax) of the time and materials not yet invoiced.
    pub unbilled_total_minor: i64,
}

async fn claim_job_for_entry(conn: &mut SqliteConnection, id: i64) -> Result<(), DataError> {
    let claimed: Option<i64> = sqlx::query_scalar(
        "UPDATE job SET status = status \
         WHERE id = ? AND deleted_at IS NULL AND status != 'invoiced' RETURNING id",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    claimed
        .map(|_| ())
        .ok_or_else(|| DataError::Other("job not found or it has already been invoiced".into()))
}

/// Longest single time entry accepted (about 694 days of minutes); bounds the labour arithmetic.
const MAX_ENTRY_MINUTES: i64 = 1_000_000;

/// Longest invoice line description (matches the document line limit).
const MAX_LINE_DESCRIPTION: usize = 2_000;

fn labour_amount(minutes: i64, rate_minor: i64) -> Result<i64, DataError> {
    Decimal::from(rate_minor)
        .checked_mul(Decimal::from(minutes))
        .and_then(|amount| amount.checked_div(Decimal::from(60)))
        .and_then(|amount| {
            amount
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_i64()
        })
        .ok_or_else(|| DataError::Other("labour amount is too large".into()))
}

/// Shorten generated text to fit a document line, marking the cut with an ellipsis.
fn fit_description(text: String) -> String {
    if text.chars().count() <= MAX_LINE_DESCRIPTION {
        return text;
    }
    let mut clipped: String = text.chars().take(MAX_LINE_DESCRIPTION - 1).collect();
    clipped.push('…');
    clipped
}

pub async fn create(db: &Db, input: &JobInput) -> Result<i64, DataError> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(DataError::Other("a job needs a title".into()));
    }
    if title.chars().count() > 200 {
        return Err(DataError::Other(
            "job title cannot exceed 200 characters".into(),
        ));
    }
    if input.description.chars().count() > 20_000 {
        return Err(DataError::Other(
            "job description cannot exceed 20,000 characters".into(),
        ));
    }
    let customer_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM customer WHERE id = ? AND deleted_at IS NULL)",
    )
    .bind(input.customer_id)
    .fetch_one(db)
    .await?;
    if !customer_exists {
        return Err(DataError::Other("active customer not found".into()));
    }
    let id = sqlx::query("INSERT INTO job (customer_id, title, description) VALUES (?, ?, ?)")
        .bind(input.customer_id)
        .bind(title)
        .bind(input.description.trim())
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

pub async fn list_for_customer(db: &Db, customer_id: i64) -> Result<Vec<Job>, DataError> {
    Ok(sqlx::query_as::<_, Job>(
        "SELECT id, customer_id, title, description, status, source_quote_id, created_at \
         FROM job WHERE customer_id = ? AND deleted_at IS NULL ORDER BY id DESC",
    )
    .bind(customer_id)
    .fetch_all(db)
    .await?)
}

/// Create a job from an **accepted** quote: the quote's lines become the job's (un-invoiced)
/// materials, the job links back via `source_quote_id`, and the quote is marked `converted` —
/// all in one transaction. Time is then logged as the work happens, and billing goes through
/// `invoice_from_job` (so the quoted scope can't also be converted straight to an invoice).
pub async fn create_from_quote(db: &Db, quote_id: i64) -> Result<i64, DataError> {
    // Claim the accepted quote before reading its lines. This competes atomically with direct
    // invoice conversion, so one accepted quote can produce exactly one downstream record.
    let mut tx = begin_write(db).await?;
    let source: Option<(i64, Option<String>, String)> = sqlx::query_as(
        "UPDATE quote SET status = 'converted' \
         WHERE id = ? AND deleted_at IS NULL AND status = 'accepted' \
         RETURNING customer_id, number, notes",
    )
    .bind(quote_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (customer_id, number, notes) = source
        .ok_or_else(|| DataError::Other("quote not found or it is no longer accepted".into()))?;
    let title = format!(
        "Job — {}",
        number.unwrap_or_else(|| format!("quote #{quote_id}"))
    );

    let job_id = sqlx::query(
        "INSERT INTO job (customer_id, title, description, source_quote_id) VALUES (?, ?, ?, ?)",
    )
    .bind(customer_id)
    .bind(&title)
    .bind(&notes)
    .bind(quote_id)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    let lines = sqlx::query_as::<_, (Option<i64>, String, String, i64, String, i64, bool)>(
        "SELECT item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, \
         tax_inclusive FROM quote_line WHERE quote_id = ? ORDER BY line_order",
    )
    .bind(quote_id)
    .fetch_all(&mut *tx)
    .await?;
    if lines.is_empty() {
        return Err(DataError::Other("accepted quote has no lines".into()));
    }
    // The materials must be billable later, so hold them to the same rules as `add_material`.
    let inputs: Vec<LineInput> = lines
        .iter()
        .map(
            |(item_id, description, quantity, price, tax_name, tax_bp, inclusive)| LineInput {
                item_id: *item_id,
                description: description.clone(),
                quantity: quantity.clone(),
                unit_price_minor: *price,
                tax_rate_name: tax_name.clone(),
                tax_rate_bp: *tax_bp,
                tax_inclusive: *inclusive,
            },
        )
        .collect();
    invoices::validated_totals(&inputs)?;
    invoices::validate_line_items(&mut tx, &inputs).await?;
    for input in &inputs {
        if let Some(item_id) = input.item_id {
            let tracked: bool = sqlx::query_scalar("SELECT tracked FROM item WHERE id = ?")
                .bind(item_id)
                .fetch_one(&mut *tx)
                .await?;
            if tracked {
                invoices::tracked_qty(&input.quantity)?;
            }
        }
    }
    for (item_id, description, quantity, price, tax_name, tax_bp, inclusive) in lines {
        sqlx::query(
            "INSERT INTO job_material (job_id, item_id, description, quantity, unit_price_minor, \
             tax_rate_name, tax_rate_bp, tax_inclusive) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(job_id)
        .bind(item_id)
        .bind(description)
        .bind(quantity)
        .bind(price)
        .bind(tax_name)
        .bind(tax_bp)
        .bind(inclusive)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(job_id)
}

pub async fn set_status(db: &Db, id: i64, status: &str) -> Result<(), DataError> {
    let current: Option<String> =
        sqlx::query_scalar("SELECT status FROM job WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(db)
            .await?;
    let current = current.ok_or_else(|| DataError::Other("job not found".into()))?;
    let from = JobStatus::from_db(&current)
        .ok_or_else(|| DataError::Other("invalid current status".into()))?;
    let target = JobStatus::from_db(status)
        .ok_or_else(|| DataError::Other(format!("unknown status '{status}'")))?;
    // `invoiced` is system-only (set when the job is billed); reject manual moves into/out of it.
    if !from.can_transition_to(target) {
        return Err(DataError::Other(format!(
            "cannot move a job from {current} to {status}"
        )));
    }
    let changed =
        sqlx::query("UPDATE job SET status = ? WHERE id = ? AND deleted_at IS NULL AND status = ?")
            .bind(target.as_db())
            .bind(id)
            .bind(&current)
            .execute(db)
            .await?
            .rows_affected();
    if changed != 1 {
        return Err(DataError::Other(
            "job changed while its status was being updated; try again".into(),
        ));
    }
    Ok(())
}

/// Delete an empty job. A job created from a quote hands that quote back (to "accepted") so it can
/// be converted again.
pub async fn delete(db: &Db, id: i64) -> Result<(), DataError> {
    let mut tx = begin_write(db).await?;
    let deleted: Option<Option<i64>> = sqlx::query_scalar(
        "UPDATE job SET deleted_at = datetime('now') \
         WHERE id = ? AND deleted_at IS NULL AND status != 'invoiced' \
           AND NOT EXISTS (SELECT 1 FROM time_entry WHERE job_id = job.id) \
           AND NOT EXISTS (SELECT 1 FROM job_material WHERE job_id = job.id) \
         RETURNING source_quote_id",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(source_quote_id) = deleted else {
        return Err(DataError::Other(
            "job not found, already invoiced, or still contains time or materials".into(),
        ));
    };
    if let Some(quote_id) = source_quote_id {
        sqlx::query(
            "UPDATE quote SET status = 'accepted' \
             WHERE id = ? AND status = 'converted' AND converted_invoice_id IS NULL",
        )
        .bind(quote_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn add_time(db: &Db, job_id: i64, t: &TimeEntryInput) -> Result<i64, DataError> {
    if !valid_business_date(&t.date) {
        return Err(DataError::Other(
            "time-entry date must be a valid YYYY-MM-DD date".into(),
        ));
    }
    if t.minutes <= 0 || t.minutes > MAX_ENTRY_MINUTES {
        return Err(DataError::Other(format!(
            "time-entry minutes must be between 1 and {MAX_ENTRY_MINUTES}"
        )));
    }
    if t.rate_minor < 0 {
        return Err(DataError::Other("hourly rate cannot be negative".into()));
    }
    if t.description.chars().count() > 2_000 {
        return Err(DataError::Other(
            "time-entry description cannot exceed 2,000 characters".into(),
        ));
    }
    let validation_line = LineInput {
        item_id: None,
        description: "Labour".into(),
        quantity: "1".into(),
        unit_price_minor: labour_amount(t.minutes, t.rate_minor)?,
        tax_rate_name: t.tax_rate_name.clone(),
        tax_rate_bp: t.tax_rate_bp,
        tax_inclusive: t.tax_inclusive,
    };
    let validated_line = invoices::to_doc_line(&validation_line)?;
    let mut tx = begin_write(db).await?;
    claim_job_for_entry(&mut tx, job_id).await?;
    let id = sqlx::query(
        "INSERT INTO time_entry (job_id, date, minutes, rate_minor, description, tax_rate_name, tax_rate_bp, tax_inclusive) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job_id)
    .bind(t.date.trim())
    .bind(t.minutes)
    .bind(t.rate_minor)
    .bind(t.description.trim())
    .bind(&validated_line.tax_rate.name)
    .bind(t.tax_rate_bp)
    .bind(t.tax_inclusive)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    tx.commit().await?;
    Ok(id)
}

pub async fn add_material(db: &Db, job_id: i64, m: &JobMaterialInput) -> Result<i64, DataError> {
    let validated_line = invoices::to_doc_line(&LineInput {
        item_id: m.item_id,
        description: m.description.clone(),
        quantity: m.quantity.clone(),
        unit_price_minor: m.unit_price_minor,
        tax_rate_name: m.tax_rate_name.clone(),
        tax_rate_bp: m.tax_rate_bp,
        tax_inclusive: m.tax_inclusive,
    })?;
    let mut tx = begin_write(db).await?;
    claim_job_for_entry(&mut tx, job_id).await?;
    if let Some(item_id) = m.item_id {
        let item: Option<(String, bool)> =
            sqlx::query_as("SELECT kind, tracked FROM item WHERE id = ? AND deleted_at IS NULL")
                .bind(item_id)
                .fetch_optional(&mut *tx)
                .await?;
        let (_, tracked) = item.ok_or_else(|| DataError::Other("active item not found".into()))?;
        if tracked {
            invoices::tracked_qty(&m.quantity)?;
        }
    }
    let id = sqlx::query(
        "INSERT INTO job_material (job_id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, tax_inclusive) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job_id)
    .bind(m.item_id)
    .bind(&validated_line.description)
    .bind(validated_line.quantity.normalize().to_string())
    .bind(m.unit_price_minor)
    .bind(&validated_line.tax_rate.name)
    .bind(m.tax_rate_bp)
    .bind(m.tax_inclusive)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    tx.commit().await?;
    Ok(id)
}

pub async fn delete_time(db: &Db, id: i64) -> Result<(), DataError> {
    let deleted = sqlx::query("DELETE FROM time_entry WHERE id = ? AND invoiced = 0")
        .bind(id)
        .execute(db)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(DataError::Other(
            "time entry not found or already invoiced".into(),
        ));
    }
    Ok(())
}

pub async fn delete_material(db: &Db, id: i64) -> Result<(), DataError> {
    let deleted = sqlx::query("DELETE FROM job_material WHERE id = ? AND invoiced = 0")
        .bind(id)
        .execute(db)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(DataError::Other(
            "material not found or already invoiced".into(),
        ));
    }
    Ok(())
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

    let mut labour_total_minor = 0_i64;
    for entry in &time_entries {
        labour_total_minor = labour_total_minor
            .checked_add(labour_amount(entry.minutes, entry.rate_minor)?)
            .ok_or_else(|| DataError::Other("labour total is too large".into()))?;
    }
    let mut materials_total_minor = 0_i64;
    for material in &materials {
        // Display must never fail on stored data: a quantity that can't be parsed counts as zero here
        // and is reported by name when the job is invoiced, so the row can still be seen and removed.
        let qty = Decimal::from_str(material.quantity.trim()).unwrap_or(Decimal::ZERO);
        let amount = Decimal::from(material.unit_price_minor)
            .checked_mul(qty)
            .and_then(|amount| {
                amount
                    .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                    .to_i64()
            })
            .ok_or_else(|| DataError::Other("material amount is too large".into()))?;
        materials_total_minor = materials_total_minor
            .checked_add(amount)
            .ok_or_else(|| DataError::Other("materials total is too large".into()))?;
    }

    // Totals with tax, per line, the same way the resulting invoice will compute them.
    let (mut subtotal_minor, mut tax_minor, mut total_minor, mut unbilled_total_minor) =
        (0_i64, 0_i64, 0_i64, 0_i64);
    let entries = time_entries
        .iter()
        .map(|t| {
            Ok((
                labour_amount(t.minutes, t.rate_minor)?,
                TaxRate::new(t.tax_rate_name.clone(), t.tax_rate_bp, t.tax_inclusive),
                t.invoiced,
            ))
        })
        .chain(materials.iter().map(|m| {
            let qty = Decimal::from_str(m.quantity.trim()).unwrap_or(Decimal::ZERO);
            let amount = Decimal::from(m.unit_price_minor)
                .checked_mul(qty)
                .and_then(|a| {
                    a.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                        .to_i64()
                })
                .ok_or_else(|| DataError::Other("material amount is too large".into()))?;
            Ok((
                amount,
                TaxRate::new(m.tax_rate_name.clone(), m.tax_rate_bp, m.tax_inclusive),
                m.invoiced,
            ))
        }))
        .collect::<Result<Vec<_>, DataError>>()?;
    for (amount, rate, invoiced) in entries {
        if !(0..=100_000).contains(&rate.rate_bp)
            || amount.unsigned_abs() > (i64::MAX / 11).unsigned_abs()
        {
            return Err(DataError::Other(
                "job contains an entry that is out of range".into(),
            ));
        }
        let split = line_tax(Money::from_minor(amount), &rate);
        let overflow = || DataError::Other("job total is too large".into());
        subtotal_minor = subtotal_minor
            .checked_add(split.net.minor())
            .ok_or_else(overflow)?;
        tax_minor = tax_minor
            .checked_add(split.tax.minor())
            .ok_or_else(overflow)?;
        total_minor = total_minor
            .checked_add(split.gross.minor())
            .ok_or_else(overflow)?;
        if !invoiced {
            unbilled_total_minor = unbilled_total_minor
                .checked_add(split.gross.minor())
                .ok_or_else(overflow)?;
        }
    }

    Ok(Some(JobDetail {
        job,
        time_entries,
        materials,
        labour_total_minor,
        materials_total_minor,
        subtotal_minor,
        tax_minor,
        total_minor,
        unbilled_total_minor,
    }))
}

/// Create a draft invoice from the job's **un-invoiced** time + materials, mark them invoiced, and
/// link the job. Returns the new invoice id. Errors if there's nothing un-invoiced.
pub async fn invoice_from_job(db: &Db, id: i64) -> Result<i64, DataError> {
    // Claim the job as the first write in this transaction. A concurrent second request waits,
    // then updates no row and exits, so the same time/materials can never produce two invoices.
    // Any later error rolls this status change back with the rest of the transaction.
    let mut tx = begin_write(db).await?;
    let customer_id: Option<i64> = sqlx::query_scalar(
        "UPDATE job SET status = 'invoiced' \
         WHERE id = ? AND deleted_at IS NULL AND status != 'invoiced' RETURNING customer_id",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let customer_id = customer_id
        .ok_or_else(|| DataError::Other("job not found or it has already been invoiced".into()))?;
    let time_entries = sqlx::query_as::<_, TimeEntry>(
        "SELECT id, date, minutes, rate_minor, description, tax_rate_name, tax_rate_bp, \
         tax_inclusive, invoiced FROM time_entry WHERE job_id = ? AND invoiced = 0 ORDER BY id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let materials = sqlx::query_as::<_, JobMaterial>(
        "SELECT id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, \
         tax_inclusive, invoiced FROM job_material WHERE job_id = ? AND invoiced = 0 ORDER BY id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;

    let mut lines: Vec<LineInput> = Vec::new();
    for t in &time_entries {
        let desc = if t.description.trim().is_empty() {
            format!("Labour ({})", t.date)
        } else {
            format!("Labour: {} ({})", t.description, t.date)
        };
        lines.push(LineInput {
            item_id: None,
            description: fit_description(format!("{desc} — {} minutes", t.minutes)),
            quantity: "1".into(),
            unit_price_minor: labour_amount(t.minutes, t.rate_minor)?,
            tax_rate_name: t.tax_rate_name.clone(),
            tax_rate_bp: t.tax_rate_bp,
            tax_inclusive: t.tax_inclusive,
        });
    }
    for m in &materials {
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

    // Atomic: the draft invoice and the "invoiced" marks (plus job status + link) commit together,
    // so a crash can't leave a billed invoice with un-marked source rows that a retry would re-bill.
    let invoice_id = invoices::create_draft_on(&mut tx, customer_id, &lines, None, "").await?;
    sqlx::query("UPDATE time_entry SET invoiced = 1 WHERE job_id = ? AND invoiced = 0")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE job_material SET invoiced = 1 WHERE job_id = ? AND invoiced = 0")
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
        let time_id = add_time(
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

        // A billed job's status is system-owned: it can't be manually moved back out of `invoiced`.
        assert!(set_status(&pool, job, "open").await.is_err());
        assert!(delete_time(&pool, time_id).await.is_err());
        assert!(add_time(
            &pool,
            job,
            &TimeEntryInput {
                date: "2026-06-06".into(),
                minutes: 30,
                rate_minor: 6000,
                description: String::new(),
                tax_rate_name: "No Tax".into(),
                tax_rate_bp: 0,
                tax_inclusive: false,
            },
        )
        .await
        .is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn time_billing_uses_exact_minutes_and_rejects_invalid_entries(
        pool: Db,
    ) -> Result<(), DataError> {
        let customer = customers::create(
            &pool,
            &customers::CustomerInput {
                name: "Minute test".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await?;
        assert!(create(
            &pool,
            &JobInput {
                customer_id: customer,
                title: "   ".into(),
                description: String::new(),
            },
        )
        .await
        .is_err());
        let job = create(
            &pool,
            &JobInput {
                customer_id: customer,
                title: "One-minute task".into(),
                description: String::new(),
            },
        )
        .await?;
        let valid = TimeEntryInput {
            date: "2028-02-29".into(),
            minutes: 1,
            rate_minor: 6000,
            description: String::new(),
            tax_rate_name: "No Tax".into(),
            tax_rate_bp: 0,
            tax_inclusive: false,
        };
        let mut invalid_date = valid.clone();
        invalid_date.date = "2027-02-29".into();
        assert!(add_time(&pool, job, &invalid_date).await.is_err());
        let mut invalid_minutes = valid.clone();
        invalid_minutes.minutes = 0;
        assert!(add_time(&pool, job, &invalid_minutes).await.is_err());
        add_time(&pool, job, &valid).await?;
        assert_eq!(
            get_detail(&pool, job).await?.unwrap().labour_total_minor,
            100
        );

        let invoice_id = invoice_from_job(&pool, job).await?;
        let invoice = invoices::get_detail(&pool, invoice_id).await?.unwrap();
        assert_eq!(invoice.invoice.total_minor, 100);
        assert_eq!(invoice.lines[0].quantity, "1");

        Ok(())
    }

    #[sqlx::test]
    async fn set_status_validates(pool: Db) -> Result<(), DataError> {
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
                title: "X".into(),
                description: String::new(),
            },
        )
        .await?;
        set_status(&pool, job, "in_progress").await?;
        set_status(&pool, job, "done").await?;
        // Unknown status, and manual jump to the system-only `invoiced`, are both rejected.
        assert!(set_status(&pool, job, "banana").await.is_err());
        assert!(set_status(&pool, job, "invoiced").await.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn job_from_accepted_quote(pool: Db) -> Result<(), DataError> {
        use crate::repos::quotes;

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
        let line = LineInput {
            item_id: None,
            description: "Turf supply".into(),
            quantity: "4".into(),
            unit_price_minor: 2500,
            tax_rate_name: "GST 10%".into(),
            tax_rate_bp: 1000,
            tax_inclusive: false,
        };
        let q = quotes::create_draft(&pool, customer, &[line], None, "front lawn").await?;

        // Not accepted yet → rejected.
        assert!(create_from_quote(&pool, q).await.is_err());

        quotes::set_status(&pool, q, "sent").await?;
        quotes::set_status(&pool, q, "accepted").await?;
        let job_id = create_from_quote(&pool, q).await?;

        let d = get_detail(&pool, job_id).await?.unwrap();
        assert_eq!(d.job.source_quote_id, Some(q));
        assert_eq!(d.materials.len(), 1);
        assert_eq!(d.materials[0].quantity, "4");
        assert_eq!(d.materials_total_minor, 10000); // 4 × $25.00
        assert_eq!(
            quotes::get_detail(&pool, q).await?.unwrap().quote.status,
            "converted"
        );
        // A converted quote can't be converted again (to a job or an invoice).
        assert!(create_from_quote(&pool, q).await.is_err());
        Ok(())
    }
}
