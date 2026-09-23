//! Recurring invoice schedules. A schedule holds a customer + line template + cadence;
//! [`run_due`] turns every elapsed period into an ordinary **draft** invoice (the user still
//! reviews and issues it), advancing `next_date` in the same transaction as each draft so a
//! crash can never double-generate. Schedules auto-deactivate once past their end date.
//!
//! Month-based schedules preserve their original day-of-month and clamp to the target month's
//! final day (31 January → 28 February → 31 March; leap years included).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::{begin_write, Db};
use crate::error::DataError;
use crate::repos::invoices::{self, to_doc_line, validate_line_items, validated_totals, LineInput};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RecurringInvoice {
    pub id: i64,
    pub customer_id: i64,
    pub frequency: String,
    pub next_date: String,
    pub end_date: Option<String>,
    pub due_days: Option<i64>,
    pub anchor_day: i64,
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
    /// Why the template cannot generate invoices as stored (edit the schedule to fix it). The list
    /// must still load so the schedule can be opened, fixed, paused or deleted.
    pub problem: Option<String>,
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
        other => {
            return Err(DataError::Other(format!(
            "unknown frequency '{other}' (use weekly, fortnightly, monthly, quarterly, or yearly)"
        )))
        }
    })
}

fn validate(input: &RecurringInput, lines: &[LineInput]) -> Result<(), DataError> {
    frequency_modifier(&input.frequency)?;
    if lines.is_empty() {
        return Err(DataError::Other(
            "a schedule needs at least one line".into(),
        ));
    }
    let (_, _, _) = date_parts(&input.next_date)
        .ok_or_else(|| DataError::Other("start date must be a valid YYYY-MM-DD date".into()))?;
    if let Some(end) = input.end_date.as_deref() {
        date_parts(end)
            .ok_or_else(|| DataError::Other("end date must be a valid YYYY-MM-DD date".into()))?;
        if end < input.next_date.as_str() {
            return Err(DataError::Other(
                "end date cannot be earlier than the start date".into(),
            ));
        }
    }
    if input
        .due_days
        .is_some_and(|days| !(0..=3_650).contains(&days))
    {
        return Err(DataError::Other(
            "invoice due days must be between 0 and 3,650".into(),
        ));
    }
    if input.notes.chars().count() > 20_000 {
        return Err(DataError::Other(
            "schedule notes cannot exceed 20,000 characters".into(),
        ));
    }
    validated_totals(lines)?;
    Ok(())
}

async fn ensure_active_customer(
    conn: &mut sqlx::SqliteConnection,
    customer_id: i64,
) -> Result<(), DataError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM customer WHERE id = ? AND deleted_at IS NULL)",
    )
    .bind(customer_id)
    .fetch_one(&mut *conn)
    .await?;
    if !exists {
        return Err(DataError::Other("active customer not found".into()));
    }
    Ok(())
}

fn date_parts(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    let (year, month, day) = (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
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

async fn next_occurrence(
    conn: &mut sqlx::SqliteConnection,
    current: &str,
    frequency: &str,
    anchor_day: i64,
) -> Result<String, DataError> {
    let day_modifier = match frequency {
        "weekly" => Some("+7 days"),
        "fortnightly" => Some("+14 days"),
        _ => None,
    };
    if let Some(modifier) = day_modifier {
        return sqlx::query_scalar::<_, Option<String>>("SELECT date(?, ?)")
            .bind(current)
            .bind(modifier)
            .fetch_one(&mut *conn)
            .await?
            .ok_or_else(|| DataError::Other("could not advance recurring date".into()));
    }

    let months = match frequency {
        "monthly" => 1,
        "quarterly" => 3,
        "yearly" => 12,
        other => return Err(DataError::Other(format!("unknown frequency '{other}'"))),
    };
    let modifier = format!("+{months} months");
    sqlx::query_scalar::<_, Option<String>>(
        "WITH target AS ( \
           SELECT date(?1, 'start of month', ?2) AS first_day, \
                  CAST(strftime('%d', date(?1, 'start of month', ?2, '+1 month', '-1 day')) AS INTEGER) AS last_day \
         ) \
         SELECT date(first_day, printf('+%d days', MIN(?3, last_day) - 1)) FROM target",
    )
    .bind(current)
    .bind(modifier)
    .bind(anchor_day.clamp(1, 31))
    .fetch_one(&mut *conn)
    .await?
    .ok_or_else(|| DataError::Other("could not advance recurring date".into()))
}

async fn insert_lines(
    conn: &mut sqlx::SqliteConnection,
    recurring_id: i64,
    lines: &[LineInput],
) -> Result<(), DataError> {
    for (i, l) in lines.iter().enumerate() {
        let line = to_doc_line(l)?;
        sqlx::query(
            "INSERT INTO recurring_invoice_line (recurring_id, item_id, description, quantity, \
             unit_price_minor, tax_rate_name, tax_rate_bp, tax_inclusive, line_order) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(recurring_id)
        .bind(l.item_id)
        .bind(&line.description)
        .bind(line.quantity.normalize().to_string())
        .bind(l.unit_price_minor)
        .bind(&line.tax_rate.name)
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
    let anchor_day = i64::from(date_parts(&input.next_date).expect("validated date").2);
    let mut tx = begin_write(db).await?;
    ensure_active_customer(&mut tx, input.customer_id).await?;
    validate_line_items(&mut tx, lines).await?;
    let id = sqlx::query(
        "INSERT INTO recurring_invoice (customer_id, frequency, next_date, end_date, due_days, anchor_day, notes) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(input.customer_id)
    .bind(&input.frequency)
    .bind(&input.next_date)
    .bind(&input.end_date)
    .bind(input.due_days)
    .bind(anchor_day)
    .bind(input.notes.trim())
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
    let anchor_day = i64::from(date_parts(&input.next_date).expect("validated date").2);
    let mut tx = begin_write(db).await?;
    ensure_active_customer(&mut tx, input.customer_id).await?;
    validate_line_items(&mut tx, lines).await?;
    let updated = sqlx::query(
        "UPDATE recurring_invoice SET customer_id = ?, frequency = ?, next_date = ?, end_date = ?, \
         due_days = ?, anchor_day = ?, notes = ? WHERE id = ?",
    )
    .bind(input.customer_id)
    .bind(&input.frequency)
    .bind(&input.next_date)
    .bind(&input.end_date)
    .bind(input.due_days)
    .bind(anchor_day)
    .bind(input.notes.trim())
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
    let mut tx = begin_write(db).await?;
    sqlx::query("DELETE FROM recurring_invoice_line WHERE recurring_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let deleted = sqlx::query("DELETE FROM recurring_invoice WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted != 1 {
        return Err(DataError::Other("schedule not found".into()));
    }
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

fn template_total(lines: &[RecurringLineRow]) -> Result<i64, DataError> {
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
    Ok(validated_totals(&inputs)?.total.minor())
}

pub async fn list(db: &Db) -> Result<Vec<RecurringListRow>, DataError> {
    let schedules = sqlx::query_as::<_, RecurringInvoice>(
        "SELECT id, customer_id, frequency, next_date, end_date, due_days, anchor_day, notes, active, \
         created_at FROM recurring_invoice ORDER BY next_date, id",
    )
    .fetch_all(db)
    .await?;
    let mut rows = Vec::with_capacity(schedules.len());
    for schedule in schedules {
        let lines = lines_for(db, schedule.id).await?;
        let (total_minor, problem) = match template_total(&lines) {
            Ok(total) => (total, None),
            Err(error) => (0, Some(error.to_string())),
        };
        rows.push(RecurringListRow {
            total_minor,
            problem,
            schedule,
        });
    }
    Ok(rows)
}

pub async fn get_detail(db: &Db, id: i64) -> Result<Option<RecurringDetail>, DataError> {
    let schedule = sqlx::query_as::<_, RecurringInvoice>(
        "SELECT id, customer_id, frequency, next_date, end_date, due_days, anchor_day, notes, active, \
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
    if date_parts(today).is_none() {
        return Err(DataError::Other(
            "run date must be a valid YYYY-MM-DD date".into(),
        ));
    }
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
            frequency_modifier(&s.frequency)?;

            if created.len() >= 10_000 {
                return Err(DataError::Other(
                    "recurring catch-up exceeded 10,000 invoices; move the schedule start date forward"
                        .into(),
                ));
            }

            // Calculate first, then atomically advance the still-expected date as the transaction's
            // first write. A concurrent run can claim a period only once; all other work rolls back
            // with this advance if draft creation fails.
            let next = {
                let mut conn = db.acquire().await?;
                next_occurrence(&mut conn, &s.next_date, &s.frequency, s.anchor_day).await?
            };

            let mut tx = begin_write(db).await?;
            let claimed = sqlx::query(
                "UPDATE recurring_invoice SET next_date = ? \
                 WHERE id = ? AND active = 1 AND next_date = ?",
            )
            .bind(&next)
            .bind(id)
            .bind(&s.next_date)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if claimed != 1 {
                tx.rollback().await?;
                continue;
            }
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

    #[sqlx::test]
    async fn month_end_anchor_survives_short_months_and_leap_years(
        pool: Db,
    ) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let monthly = create(
            &pool,
            &RecurringInput {
                customer_id: customer,
                frequency: "monthly".into(),
                next_date: "2026-01-31".into(),
                end_date: None,
                due_days: Some(0),
                notes: String::new(),
            },
            &[line(1000)],
        )
        .await?;

        let made = run_due(&pool, "2026-04-30").await?;
        assert_eq!(made.len(), 4, "Jan, Feb, Mar and Apr should each generate");
        let detail = get_detail(&pool, monthly).await?.unwrap();
        assert_eq!(detail.schedule.anchor_day, 31);
        assert_eq!(detail.schedule.next_date, "2026-05-31");
        set_active(&pool, monthly, false).await?;

        let yearly = create(
            &pool,
            &RecurringInput {
                customer_id: customer,
                frequency: "yearly".into(),
                next_date: "2024-02-29".into(),
                end_date: None,
                due_days: None,
                notes: String::new(),
            },
            &[line(1000)],
        )
        .await?;
        let made = run_due(&pool, "2027-03-01").await?;
        assert_eq!(made.len(), 4, "2024 through 2027 should each generate");
        assert_eq!(
            get_detail(&pool, yearly).await?.unwrap().schedule.next_date,
            "2028-02-29"
        );

        let invalid = RecurringInput {
            customer_id: customer,
            frequency: "monthly".into(),
            next_date: "2027-02-29".into(),
            end_date: None,
            due_days: None,
            notes: String::new(),
        };
        assert!(create(&pool, &invalid, &[line(1000)]).await.is_err());
        Ok(())
    }
}
