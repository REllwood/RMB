//! Recurring invoice schedules. A schedule holds a customer + line template + cadence;
//! [`run_due`] turns every elapsed period into an ordinary **draft** invoice (the user still
//! reviews and issues it), advancing `next_date` in the same transaction as each draft so a
//! crash can never double-generate. A schedule turns itself off once it passes its end date.
//!
//! Month-based schedules preserve their original day-of-month and clamp to the target month's
//! final day (31 January → 28 February → 31 March; leap years included).
//!
//! Generation is isolated per schedule: one schedule that can't generate (for example because its
//! customer was deleted while it was paused) is reported by name and never blocks the others.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::{begin_write, today_local, Db};
use crate::error::DataError;
use crate::repos::invoices::{self, to_doc_line, validate_line_items, validated_totals, LineInput};
use crate::validation::date_parts;

/// Most drafts one schedule may create in a single run. More than this almost always means a
/// mistyped start date or a long pause, so the schedule is reported instead of flooding drafts.
pub const MAX_CATCH_UP: usize = 100;

/// How far back a new start date may be (a year of catch-up), and how far ahead (ten years).
const MAX_START_DAYS_BACK: i64 = 366;
const MAX_START_DAYS_AHEAD: i64 = 3_660;

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

impl RecurringInvoice {
    /// The schedule has generated its last invoice (its next date is past the end date).
    pub fn ended(&self) -> bool {
        self.end_date
            .as_deref()
            .is_some_and(|end| self.next_date.as_str() > end)
    }
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
    /// Past its end date: it will not generate again unless the end date is moved.
    pub ended: bool,
    /// Kept for deleted customers so the schedule can still be identified.
    pub customer_name: String,
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

/// What a generation run did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RunReport {
    /// Draft invoices created, in order.
    pub created: Vec<i64>,
    /// One plain-language line per schedule that could not generate.
    pub problems: Vec<String>,
}

/// What resuming a paused schedule would do.
#[derive(Debug, Clone, Serialize)]
pub struct ResumePreview {
    /// Periods before today that were missed while paused (drafts that resuming would create).
    pub missed: i64,
    /// The next date if the missed periods are skipped instead.
    pub skip_to: String,
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
fn day_number(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn days_of(date: &str) -> Option<i64> {
    date_parts(date).map(|(y, m, d)| day_number(y, m, d))
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

/// Validate a schedule. `check_start_against` is today's date when the start date is new or
/// changed: it must be within a year back (catch-up) and ten years ahead (typo guard). An unchanged
/// start date on an existing schedule is left alone.
fn validate(
    input: &RecurringInput,
    lines: &[LineInput],
    check_start_against: Option<&str>,
) -> Result<(), DataError> {
    frequency_modifier(&input.frequency)?;
    if lines.is_empty() {
        return Err(DataError::Other(
            "a schedule needs at least one line".into(),
        ));
    }
    let start = days_of(&input.next_date)
        .ok_or_else(|| DataError::Other("start date must be a valid YYYY-MM-DD date".into()))?;
    if let Some(today) = check_start_against.and_then(days_of) {
        if start < today - MAX_START_DAYS_BACK {
            return Err(DataError::Other(
                "the next invoice date can be at most a year in the past; check the year".into(),
            ));
        }
        if start > today + MAX_START_DAYS_AHEAD {
            return Err(DataError::Other(
                "the next invoice date is more than ten years away; check the year".into(),
            ));
        }
    }
    if let Some(end) = input.end_date.as_deref() {
        days_of(end)
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

/// Create a schedule, validating its start date against today's local date.
pub async fn create(
    db: &Db,
    input: &RecurringInput,
    lines: &[LineInput],
) -> Result<i64, DataError> {
    let today = today_local(db).await?;
    create_as_of(db, input, lines, &today).await
}

/// [`create`] with an explicit "today" (`YYYY-MM-DD`), for deterministic callers and tests.
pub async fn create_as_of(
    db: &Db,
    input: &RecurringInput,
    lines: &[LineInput],
    today: &str,
) -> Result<i64, DataError> {
    validate(input, lines, Some(today))?;
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

/// Replace a schedule's settings and lines. Keeping the next date keeps the day-of-month anchor, so
/// editing a schedule that sits on a clamped date (28 February for a month-end schedule) doesn't
/// move it off the month end; choosing a new next date re-anchors to that date.
pub async fn update(
    db: &Db,
    id: i64,
    input: &RecurringInput,
    lines: &[LineInput],
) -> Result<(), DataError> {
    let today = today_local(db).await?;
    update_as_of(db, id, input, lines, &today).await
}

/// [`update`] with an explicit "today" (`YYYY-MM-DD`).
pub async fn update_as_of(
    db: &Db,
    id: i64,
    input: &RecurringInput,
    lines: &[LineInput],
    today: &str,
) -> Result<(), DataError> {
    let mut tx = begin_write(db).await?;
    let stored: Option<(String, i64)> =
        sqlx::query_as("SELECT next_date, anchor_day FROM recurring_invoice WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let (stored_next, stored_anchor) =
        stored.ok_or_else(|| DataError::Other("schedule not found".into()))?;
    let date_changed = stored_next != input.next_date;
    validate(input, lines, date_changed.then_some(today))?;
    let anchor_day = if date_changed {
        i64::from(date_parts(&input.next_date).expect("validated date").2)
    } else {
        stored_anchor
    };
    ensure_active_customer(&mut tx, input.customer_id).await?;
    validate_line_items(&mut tx, lines).await?;
    sqlx::query(
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
    .await?;
    sqlx::query("DELETE FROM recurring_invoice_line WHERE recurring_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    insert_lines(&mut tx, id, lines).await?;
    tx.commit().await?;
    Ok(())
}

async fn line_inputs_on(
    conn: &mut sqlx::SqliteConnection,
    id: i64,
) -> Result<Vec<LineInput>, DataError> {
    let rows = sqlx::query_as::<_, RecurringLineRow>(
        "SELECT id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, \
         tax_inclusive FROM recurring_invoice_line WHERE recurring_id = ? ORDER BY line_order",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows.iter().map(to_line_input).collect())
}

fn to_line_input(l: &RecurringLineRow) -> LineInput {
    LineInput {
        item_id: l.item_id,
        description: l.description.clone(),
        quantity: l.quantity.clone(),
        unit_price_minor: l.unit_price_minor,
        tax_rate_name: l.tax_rate_name.clone(),
        tax_rate_bp: l.tax_rate_bp,
        tax_inclusive: l.tax_inclusive,
    }
}

/// Count the periods from `next_date` that are due on or before `until` (and within `end_date`),
/// stopping once `limit` is exceeded. Returns the count and the first date after them.
async fn pending_periods(
    conn: &mut sqlx::SqliteConnection,
    schedule: &RecurringInvoice,
    until: &str,
    inclusive: bool,
    limit: usize,
) -> Result<(usize, String), DataError> {
    let mut count = 0;
    let mut date = schedule.next_date.clone();
    loop {
        let due = if inclusive {
            date.as_str() <= until
        } else {
            date.as_str() < until
        };
        let within_end = schedule
            .end_date
            .as_deref()
            .is_none_or(|end| date.as_str() <= end);
        if !due || !within_end || count > limit {
            return Ok((count, date));
        }
        count += 1;
        date = next_occurrence(conn, &date, &schedule.frequency, schedule.anchor_day).await?;
    }
}

/// What resuming a paused schedule would generate, and where skipping the missed periods would
/// continue from.
pub async fn resume_preview(db: &Db, id: i64) -> Result<ResumePreview, DataError> {
    let schedule = get_detail(db, id)
        .await?
        .ok_or_else(|| DataError::Other("schedule not found".into()))?
        .schedule;
    let today = today_local(db).await?;
    let mut conn = db.acquire().await?;
    let (missed, skip_to) = pending_periods(&mut conn, &schedule, &today, false, 10_000).await?;
    Ok(ResumePreview {
        missed: missed as i64,
        skip_to,
    })
}

/// Pause or resume a schedule. Resuming checks that it can still generate (its customer and
/// catalogue items are active and its lines are valid), so a broken schedule is fixed before it
/// runs. With `skip_missed`, periods missed while paused are skipped and the schedule continues
/// from the first date on or after today.
pub async fn set_active(
    db: &Db,
    id: i64,
    active: bool,
    skip_missed: bool,
) -> Result<(), DataError> {
    let mut tx = begin_write(db).await?;
    let schedule = sqlx::query_as::<_, RecurringInvoice>(
        "SELECT id, customer_id, frequency, next_date, end_date, due_days, anchor_day, notes, active, \
         created_at FROM recurring_invoice WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| DataError::Other("schedule not found".into()))?;
    let mut next_date = schedule.next_date.clone();
    if active {
        ensure_active_customer(&mut tx, schedule.customer_id)
            .await
            .map_err(|_| {
                DataError::Other(
                    "this schedule's customer has been deleted; edit the schedule to choose another"
                        .into(),
                )
            })?;
        let lines = line_inputs_on(&mut tx, id).await?;
        validated_totals(&lines)?;
        validate_line_items(&mut tx, &lines).await?;
        if skip_missed {
            let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
                .fetch_one(&mut *tx)
                .await?;
            let unbounded = RecurringInvoice {
                end_date: None,
                ..schedule.clone()
            };
            next_date = pending_periods(&mut tx, &unbounded, &today, false, 10_000)
                .await?
                .1;
        }
        if schedule
            .end_date
            .as_deref()
            .is_some_and(|end| next_date.as_str() > end)
        {
            return Err(DataError::Other(
                "this schedule has passed its end date; edit the end date to resume it".into(),
            ));
        }
    }
    sqlx::query("UPDATE recurring_invoice SET active = ?, next_date = ? WHERE id = ?")
        .bind(active)
        .bind(&next_date)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
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
    let inputs: Vec<LineInput> = lines.iter().map(to_line_input).collect();
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
        let (total_minor, mut problem) = match template_total(&lines) {
            Ok(total) => (total, None),
            Err(error) => (0, Some(error.to_string())),
        };
        let (customer_name, customer_deleted): (String, bool) =
            sqlx::query_as("SELECT name, deleted_at IS NOT NULL FROM customer WHERE id = ?")
                .bind(schedule.customer_id)
                .fetch_optional(db)
                .await?
                .unwrap_or_default();
        if customer_deleted && problem.is_none() {
            problem = Some("its customer has been deleted".into());
        }
        rows.push(RecurringListRow {
            total_minor,
            problem,
            ended: schedule.ended(),
            customer_name,
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
/// A schedule that fails is reported in [`RunReport::problems`] and the others still run.
pub async fn run_due(db: &Db, today: &str) -> Result<RunReport, DataError> {
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

    let mut report = RunReport::default();
    for id in due_ids {
        match generate_due(db, id, today, &mut report.created).await {
            Ok(()) => {}
            Err(error) => report.problems.push(describe_problem(db, id, &error).await),
        }
    }
    Ok(report)
}

async fn describe_problem(db: &Db, id: i64, error: &DataError) -> String {
    let who: Option<(String, String)> = sqlx::query_as(
        "SELECT COALESCE(c.name, 'a deleted customer'), r.frequency FROM recurring_invoice r \
         LEFT JOIN customer c ON c.id = r.customer_id WHERE r.id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    match who {
        Some((name, frequency)) => {
            format!("The {frequency} schedule for {name} couldn't create its invoices: {error}.")
        }
        None => format!("A recurring schedule couldn't create its invoices: {error}."),
    }
}

async fn generate_due(
    db: &Db,
    id: i64,
    today: &str,
    created: &mut Vec<i64>,
) -> Result<(), DataError> {
    // Refuse a flood before creating anything: this almost always means a wrong date.
    if let Some(detail) = get_detail(db, id).await? {
        let mut conn = db.acquire().await?;
        let (pending, _) =
            pending_periods(&mut conn, &detail.schedule, today, true, MAX_CATCH_UP).await?;
        if pending > MAX_CATCH_UP {
            return Err(DataError::Other(format!(
                "it would create more than {MAX_CATCH_UP} drafts at once; check its next date, or \
                 pause it and resume with the missed invoices skipped"
            )));
        }
    }

    // Re-read inside the loop: each generation advances next_date.
    loop {
        let Some(detail) = get_detail(db, id).await? else {
            return Ok(());
        };
        let s = &detail.schedule;
        if !s.active || s.next_date.as_str() > today {
            return Ok(());
        }
        if s.ended() {
            // Past the end: switch off (normally already done when the last period was claimed).
            sqlx::query("UPDATE recurring_invoice SET active = 0 WHERE id = ?")
                .bind(id)
                .execute(db)
                .await?;
            return Ok(());
        }
        let line_inputs: Vec<LineInput> = detail.lines.iter().map(to_line_input).collect();
        frequency_modifier(&s.frequency)?;

        // Calculate first, then atomically advance the still-expected date as the transaction's
        // first write. A concurrent run can claim a period only once; all other work rolls back
        // with this advance if draft creation fails. Advancing past the end date switches the
        // schedule off in the same write.
        let next = {
            let mut conn = db.acquire().await?;
            next_occurrence(&mut conn, &s.next_date, &s.frequency, s.anchor_day).await?
        };

        let mut tx = begin_write(db).await?;
        let claimed = sqlx::query(
            "UPDATE recurring_invoice SET next_date = ?1, \
               active = CASE WHEN end_date IS NOT NULL AND ?1 > end_date THEN 0 ELSE active END \
             WHERE id = ?2 AND active = 1 AND next_date = ?3",
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
        // Payment terms travel with the draft so issuing it late still gives the customer the
        // full term from the issue date.
        sqlx::query("UPDATE invoice SET due_days = ? WHERE id = ?")
            .bind(s.due_days)
            .bind(invoice_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        created.push(invoice_id);
    }
}

/// [`run_due`] as of the machine's local date (the normal app-startup entry point).
pub async fn run_due_now(db: &Db) -> Result<RunReport, DataError> {
    let today = today_local(db).await?;
    run_due(db, &today).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::{customers, invoices};

    /// Tests pin "today" to the schedule's own start date so they never depend on the real clock.
    async fn create(
        db: &Db,
        input: &RecurringInput,
        lines: &[LineInput],
    ) -> Result<i64, DataError> {
        super::create_as_of(db, input, lines, &input.next_date).await
    }

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
        let made = run_due(&pool, "2026-06-09").await?.created;
        assert_eq!(made.len(), 2);
        let d = get_detail(&pool, id).await?.unwrap();
        assert_eq!(d.schedule.next_date, "2026-06-15");

        let inv = invoices::get_detail(&pool, made[0]).await?.unwrap();
        assert_eq!(inv.invoice.status, "draft");
        assert_eq!(inv.invoice.total_minor, 6600);
        assert_eq!(inv.invoice.due_date.as_deref(), Some("2026-06-15")); // 06-01 + 14
        assert_eq!(inv.invoice.notes, "lawn round");

        // Re-running the same day creates nothing new (idempotent).
        assert!(run_due(&pool, "2026-06-09").await?.created.is_empty());
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
        let made = run_due(&pool, "2026-06-01").await?.created;
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
        set_active(&pool, paused, false, false).await?;
        assert!(run_due(&pool, "2026-01-20").await?.created.is_empty());

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

        let made = run_due(&pool, "2026-04-30").await?.created;
        assert_eq!(made.len(), 4, "Jan, Feb, Mar and Apr should each generate");
        let detail = get_detail(&pool, monthly).await?.unwrap();
        assert_eq!(detail.schedule.anchor_day, 31);
        assert_eq!(detail.schedule.next_date, "2026-05-31");
        set_active(&pool, monthly, false, false).await?;

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
        let made = run_due(&pool, "2027-03-01").await?.created;
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

    fn weekly(customer_id: i64, next_date: &str) -> RecurringInput {
        RecurringInput {
            customer_id,
            frequency: "weekly".into(),
            next_date: next_date.into(),
            end_date: None,
            due_days: None,
            notes: String::new(),
        }
    }

    #[sqlx::test]
    async fn one_broken_schedule_never_blocks_the_others(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let other = customers::create(
            &pool,
            &customers::CustomerInput {
                name: "Globex".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await?;
        let broken = create(&pool, &weekly(other, "2026-06-01"), &[line(1000)]).await?;
        let healthy = create(&pool, &weekly(customer, "2026-06-01"), &[line(1000)]).await?;
        // Simulate a schedule left pointing at a deleted customer by an older build.
        sqlx::query("UPDATE customer SET deleted_at = datetime('now') WHERE id = ?")
            .bind(other)
            .execute(&pool)
            .await?;

        let report = run_due(&pool, "2026-06-01").await?;
        assert_eq!(
            report.created.len(),
            1,
            "the healthy schedule still generates"
        );
        assert_eq!(report.problems.len(), 1);
        assert!(
            report.problems[0].contains("Globex"),
            "{:?}",
            report.problems
        );
        assert_eq!(
            get_detail(&pool, healthy)
                .await?
                .unwrap()
                .schedule
                .next_date,
            "2026-06-08"
        );
        assert_eq!(
            get_detail(&pool, broken).await?.unwrap().schedule.next_date,
            "2026-06-01"
        );

        // A paused schedule in that state can't be resumed until it is fixed.
        set_active(&pool, broken, false, false).await?;
        assert!(set_active(&pool, broken, true, false).await.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn resuming_can_skip_missed_periods(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let today = today_local(&pool).await?;
        let id = create_as_of(
            &pool,
            &weekly(customer, "2025-12-01"),
            &[line(1000)],
            "2025-12-01",
        )
        .await?;
        set_active(&pool, id, false, false).await?;

        let preview = resume_preview(&pool, id).await?;
        assert!(preview.missed > 0);
        assert!(preview.skip_to >= today);

        set_active(&pool, id, true, true).await?;
        let schedule = get_detail(&pool, id).await?.unwrap().schedule;
        assert!(schedule.active);
        assert_eq!(schedule.next_date, preview.skip_to);
        assert!(run_due(&pool, &today).await?.created.len() <= 1);
        Ok(())
    }

    #[sqlx::test]
    async fn editing_keeps_the_month_end_anchor(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let input = RecurringInput {
            customer_id: customer,
            frequency: "monthly".into(),
            next_date: "2026-01-31".into(),
            end_date: None,
            due_days: None,
            notes: String::new(),
        };
        let id = create(&pool, &input, &[line(1000)]).await?;
        run_due(&pool, "2026-01-31").await?;
        let on_clamped_date = get_detail(&pool, id).await?.unwrap().schedule;
        assert_eq!(on_clamped_date.next_date, "2026-02-28");

        // Edit something else while the schedule sits on 28 February.
        let edited = RecurringInput {
            next_date: on_clamped_date.next_date.clone(),
            notes: "new notes".into(),
            ..input
        };
        update_as_of(&pool, id, &edited, &[line(2000)], "2026-02-01").await?;
        run_due(&pool, "2026-02-28").await?;
        let after = get_detail(&pool, id).await?.unwrap().schedule;
        assert_eq!(after.anchor_day, 31);
        assert_eq!(after.next_date, "2026-03-31");
        Ok(())
    }

    #[sqlx::test]
    async fn start_dates_and_catch_up_are_bounded(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        // A mistyped year is rejected in both directions.
        for typo in ["0226-06-01", "2016-06-01", "2926-06-01"] {
            assert!(
                create_as_of(&pool, &weekly(customer, typo), &[line(1000)], "2026-06-01")
                    .await
                    .is_err(),
                "{typo} should be rejected"
            );
        }
        // A year of catch-up is allowed at creation.
        let id = create_as_of(
            &pool,
            &weekly(customer, "2025-06-02"),
            &[line(1000)],
            "2026-06-01",
        )
        .await?;
        // A run never floods drafts: a schedule further behind (e.g. from an older build) is
        // reported, not generated.
        sqlx::query("UPDATE recurring_invoice SET next_date = '2023-06-05' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await?;
        let report = run_due(&pool, "2026-06-01").await?;
        assert!(report.created.is_empty());
        assert_eq!(report.problems.len(), 1);
        assert_eq!(
            get_detail(&pool, id).await?.unwrap().schedule.next_date,
            "2023-06-05"
        );
        Ok(())
    }

    #[sqlx::test]
    async fn the_last_period_switches_the_schedule_off(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let id = create(
            &pool,
            &RecurringInput {
                end_date: Some("2026-06-10".into()),
                ..weekly(customer, "2026-06-01")
            },
            &[line(1000)],
        )
        .await?;
        let made = run_due(&pool, "2026-06-08").await?.created;
        assert_eq!(made.len(), 2);
        let schedule = get_detail(&pool, id).await?.unwrap().schedule;
        assert!(!schedule.active, "no more periods fall before the end date");
        assert!(schedule.ended());
        assert!(list(&pool).await?[0].ended);
        assert!(set_active(&pool, id, true, false).await.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn late_issued_catch_up_drafts_get_full_payment_terms(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let mut business = crate::repos::settings::get(&pool).await?;
        business.business_name = "Test business".into();
        crate::repos::settings::update(&pool, &business).await?;
        create(
            &pool,
            &RecurringInput {
                due_days: Some(14),
                ..weekly(customer, "2026-01-05")
            },
            &[line(1000)],
        )
        .await?;
        let made = run_due(&pool, "2026-01-05").await?.created;
        let draft = invoices::get_detail(&pool, made[0]).await?.unwrap();
        assert_eq!(draft.invoice.due_date.as_deref(), Some("2026-01-19"));

        invoices::issue(&pool, made[0]).await?;
        let today = today_local(&pool).await?;
        let expected: String = sqlx::query_scalar("SELECT date(?, '+14 days')")
            .bind(&today)
            .fetch_one(&pool)
            .await?;
        let issued = invoices::get_detail(&pool, made[0]).await?.unwrap();
        assert_eq!(issued.invoice.due_date, Some(expected));
        Ok(())
    }
}
