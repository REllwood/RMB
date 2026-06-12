//! Recurring invoice schedules. A schedule holds a customer + line template + cadence;
//! [`run_due`] turns every elapsed period into an ordinary **draft** invoice (the user still
//! reviews and issues it), advancing `next_date` in the same transaction as each draft so a
//! crash can never double-generate. Schedules auto-deactivate once past their end date.
//!
//! Note: SQLite's `+1 month` normalizes overflow (Jan 31 → Mar 3), so monthly schedules
//! anchored on the 29th–31st drift to early-month after a short month. Anchor on days 1–28
//! for stable monthly dates.

use rmb_domain::document::total_lines;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;
use crate::repos::invoices::{self, to_doc_line, LineInput};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RecurringInvoice {
    pub id: i64,
    pub customer_id: i64,
    pub frequency: String,
    pub next_date: String,
    pub end_date: Option<String>,
    pub due_days: Option<i64>,
    pub notes: String,
    pub active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RecurringLineRow {
    pub id: i64,
    pub item_id: Option<i64>,
    pub description: String,
    pub quantity: String,
    pub unit_price_minor: i64,
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub tax_inclusive: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecurringListRow {
    #[serde(flatten)]
    pub schedule: RecurringInvoice,
    pub total_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecurringDetail {
    pub schedule: RecurringInvoice,
    pub lines: Vec<RecurringLineRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecurringInput {
    pub customer_id: i64,
    pub frequency: String,
    pub next_date: String,
    pub end_date: Option<String>,
    pub due_days: Option<i64>,
    pub notes: String,
}

/// SQLite date modifier for a cadence; rejects unknown frequencies.
fn frequency_modifier(frequency: &str) -> Result<&'static str, DataError> {
    Ok(match frequency {
        "weekly" => "+7 days",
        "fortnightly" => "+14 days",
        "monthly" => "+1 month",
        "quarterly" => "+3 months",
        "yearly" => "+1 year",
        other => return Err(DataError::Other(format!(
            "unknown frequency '{other}' (use weekly, fortnightly, monthly, quarterly, or yearly)"
        ))),
    })
}

fn validate(input: &RecurringInput, lines: &[LineInput]) -> Result<(), DataError> {
    frequency_modifier(&input.frequency)?;
    if lines.is_empty() {
        return Err(DataError::Other(
            "a schedule needs at least one line".into(),
        ));
    }
    if input.next_date.is_empty() {
        return Err(DataError::Other("a schedule needs a start date".into()));
    }
    Ok(())
}

async fn insert_lines(
    conn: &mut sqlx::SqliteConnection,
    recurring_id: i64,
    lines: &[LineInput],
) -> Result<(), DataError> {
    for (i, l) in lines.iter().enumerate() {
        to_doc_line(l)?; // validate quantity parses before storing the template
        sqlx::query(
            "INSERT INTO recurring_invoice_line (recurring_id, item_id, description, quantity, \
             unit_price_minor, tax_rate_name, tax_rate_bp, tax_inclusive, line_order) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(recurring_id)
        .bind(l.item_id)
        .bind(&l.description)
        .bind(&l.quantity)
        .bind(l.unit_price_minor)
        .bind(&l.tax_rate_name)
        .bind(l.tax_rate_bp)
        .bind(l.tax_inclusive)
        .bind(i as i64)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn create(
    db: &Db,
    input: &RecurringInput,
    lines: &[LineInput],
) -> Result<i64, DataError> {
    validate(input, lines)?;
    let mut tx = db.begin().await?;
    let id = sqlx::query(
        "INSERT INTO recurring_invoice (customer_id, frequency, next_date, end_date, due_days, notes) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(input.customer_id)
    .bind(&input.frequency)
    .bind(&input.next_date)
    .bind(&input.end_date)
    .bind(input.due_days)
    .bind(&input.notes)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    insert_lines(&mut tx, id, lines).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn update(
    db: &Db,
    id: i64,
    input: &RecurringInput,
    lines: &[LineInput],
) -> Result<(), DataError> {
    validate(input, lines)?;
    let mut tx = db.begin().await?;
    let updated = sqlx::query(
        "UPDATE recurring_invoice SET customer_id = ?, frequency = ?, next_date = ?, end_date = ?, \
         due_days = ?, notes = ? WHERE id = ?",
    )
    .bind(input.customer_id)
    .bind(&input.frequency)
    .bind(&input.next_date)
    .bind(&input.end_date)
    .bind(input.due_days)
    .bind(&input.notes)
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(DataError::Other("schedule not found".into()));
    }
    sqlx::query("DELETE FROM recurring_invoice_line WHERE recurring_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    insert_lines(&mut tx, id, lines).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn set_active(db: &Db, id: i64, active: bool) -> Result<(), DataError> {
    let n = sqlx::query("UPDATE recurring_invoice SET active = ? WHERE id = ?")
        .bind(active)
        .bind(id)
        .execute(db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(DataError::Other("schedule not found".into()));
    }
    Ok(())
}

/// Schedules are templates, not documents — hard delete is fine (generated invoices keep living).
pub async fn delete(db: &Db, id: i64) -> Result<(), DataError> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM recurring_invoice_line WHERE recurring_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM recurring_invoice WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn lines_for(db: &Db, id: i64) -> Result<Vec<RecurringLineRow>, DataError> {
    Ok(sqlx::query_as::<_, RecurringLineRow>(
        "SELECT id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, \
         tax_inclusive FROM recurring_invoice_line WHERE recurring_id = ? ORDER BY line_order",
    )
    .bind(id)
    .fetch_all(db)
    .await?)
}

fn template_total(lines: &[RecurringLineRow]) -> i64 {
    let inputs: Vec<LineInput> = lines
        .iter()
        .map(|l| LineInput {
            item_id: l.item_id,
            description: l.description.clone(),
            quantity: l.quantity.clone(),
            unit_price_minor: l.unit_price_minor,
            tax_rate_name: l.tax_rate_name.clone(),
            tax_rate_bp: l.tax_rate_bp,
            tax_inclusive: l.tax_inclusive,
        })
        .collect();
    let doc_lines: Vec<_> = inputs.iter().filter_map(|l| to_doc_line(l).ok()).collect();
    total_lines(&doc_lines).total.minor()
}

pub async fn list(db: &Db) -> Result<Vec<RecurringListRow>, DataError> {
    let schedules = sqlx::query_as::<_, RecurringInvoice>(
        "SELECT id, customer_id, frequency, next_date, end_date, due_days, notes, active, \
         created_at FROM recurring_invoice ORDER BY next_date, id",
    )
    .fetch_all(db)
    .await?;
    let mut rows = Vec::with_capacity(schedules.len());
    for schedule in schedules {
        let lines = lines_for(db, schedule.id).await?;
        rows.push(RecurringListRow {
            total_minor: template_total(&lines),
            schedule,
        });
    }
    Ok(rows)
}

pub async fn get_detail(db: &Db, id: i64) -> Result<Option<RecurringDetail>, DataError> {
    let schedule = sqlx::query_as::<_, RecurringInvoice>(
        "SELECT id, customer_id, frequency, next_date, end_date, due_days, notes, active, \
         created_at FROM recurring_invoice WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    let Some(schedule) = schedule else {
        return Ok(None);
    };
    let lines = lines_for(db, id).await?;
    Ok(Some(RecurringDetail { schedule, lines }))
}

/// Generate draft invoices for every schedule period elapsed up to `today` (inclusive).
/// One transaction per generated draft: the draft and the advanced `next_date` commit together.
/// Returns the created invoice ids.
pub async fn run_due(db: &Db, today: &str) -> Result<Vec<i64>, DataError> {
    let due_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM recurring_invoice WHERE active = 1 AND next_date <= ? ORDER BY id",
    )
    .bind(today)
    .fetch_all(db)
    .await?;

    let mut created = Vec::new();
    for id in due_ids {
        // Re-read inside the loop: each generation advances next_date.
        loop {
            let Some(detail) = get_detail(db, id).await? else {
                break;
            };
            let s = &detail.schedule;
            if !s.active || s.next_date.as_str() > today {
                break;
            }
            if let Some(end) = &s.end_date {
                if s.next_date.as_str() > end.as_str() {
                    // Past the end: deactivate and stop generating.
                    set_active(db, id, false).await?;
                    break;
                }
            }

            let line_inputs: Vec<LineInput> = detail
                .lines
                .iter()
                .map(|l| LineInput {
                    item_id: l.item_id,
                    description: l.description.clone(),
                    quantity: l.quantity.clone(),
                    unit_price_minor: l.unit_price_minor,
                    tax_rate_name: l.tax_rate_name.clone(),
                    tax_rate_bp: l.tax_rate_bp,
                    tax_inclusive: l.tax_inclusive,
                })
                .collect();
            let modifier = frequency_modifier(&s.frequency)?;

            let mut tx = db.begin().await?;
            let due_date: Option<String> = match s.due_days {
                Some(days) => Some(
                    sqlx::query_scalar("SELECT date(?, ?)")
                        .bind(&s.next_date)
                        .bind(format!("+{days} days"))
                        .fetch_one(&mut *tx)
                        .await?,
                ),
                None => None,
            };
            let invoice_id = invoices::create_draft_on(
                &mut tx,
                s.customer_id,
                &line_inputs,
                due_date.as_deref(),
                &s.notes,
            )
            .await?;
            let next: String = sqlx::query_scalar("SELECT date(?, ?)")
                .bind(&s.next_date)
                .bind(modifier)
                .fetch_one(&mut *tx)
                .await?;
            sqlx::query("UPDATE recurring_invoice SET next_date = ? WHERE id = ?")
                .bind(&next)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            created.push(invoice_id);
        }
    }
    Ok(created)
}

/// [`run_due`] as of the machine's local date (the normal app-startup entry point).
pub async fn run_due_now(db: &Db) -> Result<Vec<i64>, DataError> {
    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(db)
        .await?;
    run_due(db, &today).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::customers;

    fn line(price: i64) -> LineInput {
        LineInput {
            item_id: None,
            description: "Mowing — fortnightly round".into(),
            quantity: "1".into(),
            unit_price_minor: price,
            tax_rate_name: "GST 10%".into(),
            tax_rate_bp: 1000,
            tax_inclusive: false,
        }
    }

    async fn seed_customer(db: &Db) -> i64 {
        customers::create(
            db,
            &customers::CustomerInput {
                name: "Acme".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn generates_and_advances_with_catchup(pool: Db) -> Result<(), DataError> {
        let c = seed_customer(&pool).await;
        let id = create(
            &pool,
            &RecurringInput {
                customer_id: c,
                frequency: "weekly".into(),
                next_date: "2026-06-01".into(),
                end_date: None,
                due_days: Some(14),
                notes: "lawn round".into(),
            },
            &[line(6000)],
        )
        .await?;

        // List shows the tax-correct preview total.
        let rows = list(&pool).await?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].total_minor, 6600); // $60 + GST

        // Two elapsed weeks → two drafts (catch-up), next_date lands beyond today.
        let made = run_due(&pool, "2026-06-09").await?;
        assert_eq!(made.len(), 2);
        let d = get_detail(&pool, id).await?.unwrap();
        assert_eq!(d.schedule.next_date, "2026-06-15");

        let inv = invoices::get_detail(&pool, made[0]).await?.unwrap();
        assert_eq!(inv.invoice.status, "draft");
        assert_eq!(inv.invoice.total_minor, 6600);
        assert_eq!(inv.invoice.due_date.as_deref(), Some("2026-06-15")); // 06-01 + 14
        assert_eq!(inv.invoice.notes, "lawn round");

        // Re-running the same day creates nothing new (idempotent).
        assert!(run_due(&pool, "2026-06-09").await?.is_empty());
        Ok(())
    }

    #[sqlx::test]
    async fn respects_end_date_and_active_flag(pool: Db) -> Result<(), DataError> {
        let c = seed_customer(&pool).await;
        let id = create(
            &pool,
            &RecurringInput {
                customer_id: c,
                frequency: "monthly".into(),
                next_date: "2026-01-15".into(),
                end_date: Some("2026-02-28".into()),
                due_days: None,
                notes: String::new(),
            },
            &[line(10000)],
        )
        .await?;

        // Jan + Feb generate; March would be past end_date → auto-deactivated.
        let made = run_due(&pool, "2026-06-01").await?;
        assert_eq!(made.len(), 2);
        let d = get_detail(&pool, id).await?.unwrap();
        assert!(!d.schedule.active);
        assert_eq!(d.schedule.next_date, "2026-03-15"); // advanced past end, then stopped

        // Paused schedules generate nothing.
        let paused = create(
            &pool,
            &RecurringInput {
                customer_id: c,
                frequency: "weekly".into(),
                next_date: "2026-01-01".into(),
                end_date: None,
                due_days: None,
                notes: String::new(),
            },
            &[line(500)],
        )
        .await?;
        set_active(&pool, paused, false).await?;
        assert!(run_due(&pool, "2026-01-20").await?.is_empty());

        // Unknown frequency rejected at create.
        assert!(create(
            &pool,
            &RecurringInput {
                customer_id: c,
                frequency: "daily".into(),
                next_date: "2026-01-01".into(),
                end_date: None,
                due_days: None,
                notes: String::new(),
            },
            &[line(500)],
        )
        .await
        .is_err());
        Ok(())
    }
}
