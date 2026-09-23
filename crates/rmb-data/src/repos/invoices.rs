//! Invoice repository.
//!
//! Drafts are editable and store computed line/total breakdowns. **Issuing** an invoice, in one
//! transaction: assigns the next gapless number, freezes business + customer snapshots, decrements
//! tracked-product stock exactly once, and flips status to `issued`. Issued invoices are immutable —
//! correct via **void** (reverses stock; allowed only while the invoice is unpaid).

use std::collections::HashSet;
use std::str::FromStr;

use rmb_domain::document::{total_lines, DocumentLine};
use rmb_domain::inventory::MovementReason;
use rmb_domain::numbering::format_number;
use rmb_domain::status::{payment_status, InvoiceStatus};
use rmb_domain::tax::{line_tax, DocumentTotals, TaxRate};
use rmb_domain::Money;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::{begin_write, Db};
use crate::error::DataError;
use crate::repos::items;
use crate::validation::valid_business_date;

#[derive(Debug, Clone, Deserialize)]
pub struct LineInput {
    pub item_id: Option<i64>,
    pub description: String,
    pub quantity: String, // exact decimal as string
    pub unit_price_minor: i64,
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub tax_inclusive: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InvoiceRow {
    pub id: i64,
    pub customer_id: i64,
    pub number: Option<String>,
    pub status: String,
    pub issue_date: Option<String>,
    pub due_date: Option<String>,
    pub subtotal_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub notes: String,
    pub created_at: String,
    /// The accepted quote this invoice was converted from, if any.
    pub source_quote_id: Option<i64>,
    /// The job this invoice bills, if any.
    pub source_job_id: Option<i64>,
    /// Local date the invoice was voided, if it was.
    pub void_date: Option<String>,
    /// The customer's name as frozen on the issued invoice, or the current name for drafts. Deleted
    /// customers keep their name here so historical documents never lose who they were for.
    pub customer_name: String,
}

/// `SELECT` for [`InvoiceRow`] (as a macro so queries stay static strings for sqlx). Callers append
/// `WHERE`/`ORDER BY` against the aliases `i` (invoice) and `c` (customer).
macro_rules! invoice_row_select {
    () => {
        "SELECT i.id, i.customer_id, i.number, i.status, i.issue_date, i.due_date, \
         i.subtotal_minor, i.tax_minor, i.total_minor, i.notes, i.created_at, i.source_quote_id, \
         i.source_job_id, i.void_date, \
         COALESCE(CASE WHEN json_valid(i.customer_snapshot) \
                       THEN json_extract(i.customer_snapshot, '$.name') END, c.name, '') \
           AS customer_name \
         FROM invoice i LEFT JOIN customer c ON c.id = i.customer_id"
    };
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InvoiceLineRow {
    pub id: i64,
    pub item_id: Option<i64>,
    pub description: String,
    pub quantity: String,
    pub unit_price_minor: i64,
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub tax_inclusive: bool,
    pub net_minor: i64,
    pub tax_minor: i64,
    pub gross_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceDetail {
    pub invoice: InvoiceRow,
    pub lines: Vec<InvoiceLineRow>,
    pub amount_paid_minor: i64,
    /// Frozen JSON captured when the invoice was issued. Drafts have no snapshot.
    pub business_snapshot: Option<String>,
    /// Frozen JSON captured when the invoice was issued. Drafts have no snapshot.
    pub customer_snapshot: Option<String>,
    /// Frozen per-rate tax breakdown computed with the invoice lines.
    pub tax_summary: String,
    /// Frozen logo bytes captured at issue. Not sent across IPC; only native PDF export consumes it.
    #[serde(skip_serializing)]
    pub business_logo_data: Option<Vec<u8>>,
    /// `png` or `jpg`, paired with `business_logo_data`.
    #[serde(skip_serializing)]
    pub business_logo_format: Option<String>,
}

pub(crate) fn to_doc_line(l: &LineInput) -> Result<DocumentLine, DataError> {
    let description = l.description.trim();
    if description.is_empty() {
        return Err(DataError::Other("every line needs a description".into()));
    }
    if description.chars().count() > 2_000 {
        return Err(DataError::Other(
            "line descriptions cannot exceed 2,000 characters".into(),
        ));
    }
    let quantity = l.quantity.trim();
    if quantity.chars().count() > 100 {
        return Err(DataError::Other("line quantity is too long".into()));
    }
    let qty = Decimal::from_str(quantity)
        .map_err(|e| DataError::Other(format!("invalid quantity '{}': {e}", l.quantity)))?;
    if qty <= Decimal::ZERO || qty > Decimal::from(1_000_000_000_i64) {
        return Err(DataError::Other(
            "line quantity must be greater than zero and no more than 1,000,000,000".into(),
        ));
    }
    // Negative prices are discount lines; the document total is checked in `validated_totals`.
    let tax_rate_name = l.tax_rate_name.trim();
    if tax_rate_name.is_empty() {
        return Err(DataError::Other("every line needs a tax-rate name".into()));
    }
    if tax_rate_name.chars().count() > 100 {
        return Err(DataError::Other(
            "tax-rate names cannot exceed 100 characters".into(),
        ));
    }
    // This broad upper bound still permits unusual excise rates while ruling out invalid values
    // that could overflow document arithmetic (100_000 bp = 1,000%).
    if !(0..=100_000).contains(&l.tax_rate_bp) {
        return Err(DataError::Other(
            "tax rate must be between 0% and 1,000%".into(),
        ));
    }
    let rounded_amount = Decimal::from(l.unit_price_minor)
        .checked_mul(qty)
        .and_then(|amount| {
            amount
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_i64()
        })
        .ok_or_else(|| DataError::Other("line amount is too large".into()))?;
    // Exclusive tax can expand the amount by at most 11× under the rate bound above.
    if rounded_amount.unsigned_abs() > (i64::MAX / 11).unsigned_abs() {
        return Err(DataError::Other("line amount is too large".into()));
    }
    Ok(DocumentLine::new(
        description.to_owned(),
        qty,
        Money::from_minor(l.unit_price_minor),
        TaxRate::new(tax_rate_name.to_owned(), l.tax_rate_bp, l.tax_inclusive),
    ))
}

/// Validate every line and the aggregate arithmetic before the infallible domain calculator runs.
/// This turns malformed/hostile IPC input into an ordinary error rather than a process panic.
pub(crate) fn validated_totals(lines: &[LineInput]) -> Result<DocumentTotals, DataError> {
    if lines.len() > 1_000 {
        return Err(DataError::Other(
            "a document cannot contain more than 1,000 lines".into(),
        ));
    }
    let doc_lines = lines
        .iter()
        .map(to_doc_line)
        .collect::<Result<Vec<_>, _>>()?;

    let mut net = 0_i64;
    let mut tax = 0_i64;
    let mut gross = 0_i64;
    for line in &doc_lines {
        let line_total = line_tax(line.line_amount(), &line.tax_rate);
        net = net
            .checked_add(line_total.net.minor())
            .ok_or_else(|| DataError::Other("document subtotal is too large".into()))?;
        tax = tax
            .checked_add(line_total.tax.minor())
            .ok_or_else(|| DataError::Other("document tax total is too large".into()))?;
        gross = gross
            .checked_add(line_total.gross.minor())
            .ok_or_else(|| DataError::Other("document total is too large".into()))?;
    }
    debug_assert_eq!(net.checked_add(tax), Some(gross));
    if gross < 0 {
        return Err(DataError::Other(
            "the document total cannot be negative; discount lines must not exceed the other lines"
                .into(),
        ));
    }
    Ok(total_lines(&doc_lines))
}

/// Any non-null catalogue link must point at an active item. Custom lines use `None`. Repeating
/// IDs are checked once so a large document does not issue redundant queries.
pub(crate) async fn validate_line_items(
    conn: &mut sqlx::SqliteConnection,
    lines: &[LineInput],
) -> Result<(), DataError> {
    let item_ids: HashSet<i64> = lines.iter().filter_map(|line| line.item_id).collect();
    for item_id in item_ids {
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM item WHERE id = ? AND deleted_at IS NULL)",
        )
        .bind(item_id)
        .fetch_one(&mut *conn)
        .await?;
        if !active {
            return Err(DataError::Other(format!(
                "catalogue item {item_id} was not found or is inactive"
            )));
        }
    }
    Ok(())
}

/// Quantity for a tracked-product stock movement: must be a **whole, non-negative** number.
/// Tracked stock is counted in whole units, so fractional/negative quantities are rejected
/// rather than silently rounded (which would desync stock from what was billed).
pub(crate) fn tracked_qty(qty_str: &str) -> Result<i64, DataError> {
    let qty = Decimal::from_str(qty_str.trim())
        .map_err(|e| DataError::Other(format!("invalid quantity '{qty_str}': {e}")))?;
    if qty <= Decimal::ZERO || qty.fract() != Decimal::ZERO {
        return Err(DataError::Other(format!(
            "a tracked product line needs a whole, positive quantity (got {qty})"
        )));
    }
    qty.to_i64()
        .ok_or_else(|| DataError::Other("tracked quantity is too large".into()))
}

pub(crate) async fn validate_draft_metadata(
    conn: &mut sqlx::SqliteConnection,
    customer_id: i64,
    date: Option<&str>,
    date_label: &str,
    notes: &str,
) -> Result<(), DataError> {
    let customer_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM customer WHERE id = ? AND deleted_at IS NULL)",
    )
    .bind(customer_id)
    .fetch_one(&mut *conn)
    .await?;
    if !customer_exists {
        return Err(DataError::Other("active customer not found".into()));
    }
    if date.is_some_and(|value| !valid_business_date(value.trim())) {
        return Err(DataError::Other(format!(
            "{date_label} must be a valid YYYY-MM-DD date"
        )));
    }
    if notes.chars().count() > 20_000 {
        return Err(DataError::Other(
            "document notes cannot exceed 20,000 characters".into(),
        ));
    }
    Ok(())
}

/// Create a draft invoice on a **caller-supplied** connection/transaction (does not commit), so a
/// caller can make a larger operation atomic — quote conversion and invoice-from-job both create
/// the draft and mark their source rows in one transaction, closing the crash-between-commits window
/// that would otherwise allow the same work to be billed twice.
pub(crate) async fn create_draft_on(
    conn: &mut sqlx::SqliteConnection,
    customer_id: i64,
    lines: &[LineInput],
    due_date: Option<&str>,
    notes: &str,
) -> Result<i64, DataError> {
    if lines.is_empty() {
        return Err(DataError::Other(
            "an invoice needs at least one line".into(),
        ));
    }
    let totals = validated_totals(lines)?;
    let tax_summary = serde_json::to_string(&totals.tax_summary).unwrap_or_else(|_| "[]".into());
    validate_draft_metadata(conn, customer_id, due_date, "due date", notes).await?;
    validate_line_items(conn, lines).await?;
    let due_date = due_date.map(str::trim);

    let invoice_id = sqlx::query(
        "INSERT INTO invoice (customer_id, status, due_date, subtotal_minor, tax_minor, total_minor, tax_summary, notes) \
         VALUES (?, 'draft', ?, ?, ?, ?, ?, ?)",
    )
    .bind(customer_id)
    .bind(due_date)
    .bind(totals.subtotal.minor())
    .bind(totals.tax_total.minor())
    .bind(totals.total.minor())
    .bind(tax_summary)
    .bind(notes)
    .execute(&mut *conn)
    .await?
    .last_insert_rowid();

    insert_lines(conn, invoice_id, lines).await?;
    Ok(invoice_id)
}

/// Insert the computed line rows for an invoice (shared by create + update-draft).
async fn insert_lines(
    conn: &mut sqlx::SqliteConnection,
    invoice_id: i64,
    lines: &[LineInput],
) -> Result<(), DataError> {
    for (i, l) in lines.iter().enumerate() {
        let dl = to_doc_line(l)?;
        let lt = line_tax(dl.line_amount(), &dl.tax_rate);
        sqlx::query(
            "INSERT INTO invoice_line (invoice_id, item_id, description, quantity, unit_price_minor, \
             tax_rate_name, tax_rate_bp, tax_inclusive, net_minor, tax_minor, gross_minor, line_order) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(invoice_id)
        .bind(l.item_id)
        .bind(&dl.description)
        .bind(dl.quantity.normalize().to_string())
        .bind(l.unit_price_minor)
        .bind(&dl.tax_rate.name)
        .bind(l.tax_rate_bp)
        .bind(l.tax_inclusive)
        .bind(lt.net.minor())
        .bind(lt.tax.minor())
        .bind(lt.gross.minor())
        .bind(i as i64)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Create a draft invoice from line inputs in its own transaction.
pub async fn create_draft(
    db: &Db,
    customer_id: i64,
    lines: &[LineInput],
    due_date: Option<&str>,
    notes: &str,
) -> Result<i64, DataError> {
    let mut tx = begin_write(db).await?;
    let invoice_id = create_draft_on(&mut tx, customer_id, lines, due_date, notes).await?;
    tx.commit().await?;
    Ok(invoice_id)
}

/// Replace a **draft** invoice's customer, lines, due date, and notes (recomputing totals).
/// Issued invoices are immutable — anything past draft is rejected.
pub async fn update_draft(
    db: &Db,
    id: i64,
    customer_id: i64,
    lines: &[LineInput],
    due_date: Option<&str>,
    notes: &str,
) -> Result<(), DataError> {
    if lines.is_empty() {
        return Err(DataError::Other(
            "an invoice needs at least one line".into(),
        ));
    }
    let totals = validated_totals(lines)?;
    let tax_summary = serde_json::to_string(&totals.tax_summary).unwrap_or_else(|_| "[]".into());

    let mut tx = begin_write(db).await?;
    let claimed: Option<(i64, Option<i64>, Option<i64>)> = sqlx::query_as(
        "UPDATE invoice SET status = status WHERE id = ? AND status = 'draft' \
         RETURNING customer_id, source_quote_id, source_job_id",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((current_customer, source_quote_id, source_job_id)) = claimed else {
        return Err(DataError::Other(
            "only draft invoices can be edited (void + reissue to correct an issued one)".into(),
        ));
    };
    if customer_id != current_customer && (source_quote_id.is_some() || source_job_id.is_some()) {
        return Err(DataError::Other(
            "this draft was created from a quote or job, so it stays with that customer".into(),
        ));
    }
    validate_draft_metadata(&mut tx, customer_id, due_date, "due date", notes).await?;
    validate_line_items(&mut tx, lines).await?;
    let due_date = due_date.map(str::trim);

    sqlx::query(
        "UPDATE invoice SET customer_id = ?, \
         due_days = CASE WHEN due_date IS ? THEN due_days ELSE NULL END, due_date = ?, \
         notes = ?, subtotal_minor = ?, tax_minor = ?, total_minor = ?, tax_summary = ? \
         WHERE id = ?",
    )
    .bind(customer_id)
    .bind(due_date)
    .bind(due_date)
    .bind(notes)
    .bind(totals.subtotal.minor())
    .bind(totals.tax_total.minor())
    .bind(totals.total.minor())
    .bind(tax_summary)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM invoice_line WHERE invoice_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    insert_lines(&mut tx, id, lines).await?;
    tx.commit().await?;
    Ok(())
}

/// Make the quote or job an invoice came from billable again, inside the caller's transaction.
/// Used when that invoice is deleted as a draft or voided: the job's time and materials return to
/// unbilled and the job to "done", and the quote returns to "accepted" so it can be converted again.
async fn release_sources(
    conn: &mut sqlx::SqliteConnection,
    invoice_id: i64,
    source_quote_id: Option<i64>,
    source_job_id: Option<i64>,
) -> Result<(), DataError> {
    if let Some(job_id) = source_job_id {
        sqlx::query("UPDATE time_entry SET invoiced = 0 WHERE job_id = ? AND invoiced = 1")
            .bind(job_id)
            .execute(&mut *conn)
            .await?;
        sqlx::query("UPDATE job_material SET invoiced = 0 WHERE job_id = ? AND invoiced = 1")
            .bind(job_id)
            .execute(&mut *conn)
            .await?;
        sqlx::query("UPDATE job SET status = 'done' WHERE id = ? AND status = 'invoiced'")
            .bind(job_id)
            .execute(&mut *conn)
            .await?;
    }
    if let Some(quote_id) = source_quote_id {
        sqlx::query(
            "UPDATE quote SET status = 'accepted', converted_invoice_id = NULL \
             WHERE id = ? AND status = 'converted' AND converted_invoice_id = ?",
        )
        .bind(quote_id)
        .bind(invoice_id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Hard-delete a **draft** invoice (and its lines). Drafts have no number, no stock effect, and
/// no payments, so deletion is safe; issued invoices are voided, never deleted. A draft created
/// from a quote or job hands that work back so it can be billed again.
pub async fn delete_draft(db: &Db, id: i64) -> Result<(), DataError> {
    let mut tx = begin_write(db).await?;
    let claimed: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
        "UPDATE invoice SET status = status WHERE id = ? AND status = 'draft' \
         RETURNING source_quote_id, source_job_id",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((source_quote_id, source_job_id)) = claimed else {
        return Err(DataError::Other(
            "only draft invoices can be deleted (issued invoices are voided instead)".into(),
        ));
    };
    release_sources(&mut tx, id, source_quote_id, source_job_id).await?;
    sqlx::query("DELETE FROM invoice_line WHERE invoice_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM invoice WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// The first number from `seq` that no document in `table` already uses. A prefix change (for
/// example "INV-1" to "INV-") can make the counter produce a number issued long ago; skipping it
/// keeps numbers unique instead of blocking every future issue.
pub(crate) async fn next_free_number(
    conn: &mut sqlx::SqliteConnection,
    table: &str,
    prefix: &str,
    mut seq: i64,
    pad: i64,
) -> Result<(String, i64), DataError> {
    let exists_sql = match table {
        "invoice" => "SELECT EXISTS(SELECT 1 FROM invoice WHERE number = ?)",
        "quote" => "SELECT EXISTS(SELECT 1 FROM quote WHERE number = ?)",
        other => {
            return Err(DataError::Other(format!(
                "unknown numbered table '{other}'"
            )))
        }
    };
    for _ in 0..100_000 {
        let candidate = format_number(prefix, seq, pad.max(0) as usize);
        let taken: bool = sqlx::query_scalar(exists_sql)
            .bind(&candidate)
            .fetch_one(&mut *conn)
            .await?;
        if !taken {
            return Ok((candidate, seq));
        }
        seq += 1;
    }
    Err(DataError::Other(
        "no unused document number was found; change the prefix in Settings".into(),
    ))
}

/// Issue a draft: assign number, freeze snapshots, decrement stock once, set status = issued.
pub async fn issue(db: &Db, id: i64) -> Result<(), DataError> {
    let mut tx = begin_write(db).await?;

    // Claim the draft as the transaction's first operation. Duplicate issue/edit/delete actions
    // therefore serialize before numbering, snapshots, or stock movements are touched.
    let row = sqlx::query_as::<_, (i64, i64)>(
        "UPDATE invoice SET status = status WHERE id = ? AND status = 'draft' \
         RETURNING customer_id, total_minor",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (customer_id, total_minor) =
        row.ok_or_else(|| DataError::Other("invoice not found or it is no longer a draft".into()))?;
    let line_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM invoice_line WHERE invoice_id = ?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if line_count == 0 {
        return Err(DataError::Other(
            "cannot issue an invoice with no lines".into(),
        ));
    }

    // Gapless number from the settings counter (read + increment in-transaction).
    let (prefix, seq, pad) = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT invoice_prefix, invoice_next_seq, number_pad FROM settings WHERE id = 1",
    )
    .fetch_one(&mut *tx)
    .await?;
    let (number, seq) = next_free_number(&mut tx, "invoice", &prefix, seq, pad).await?;
    sqlx::query("UPDATE settings SET invoice_next_seq = ? WHERE id = 1")
        .bind(seq + 1)
        .execute(&mut *tx)
        .await?;

    // Freeze business + customer snapshots.
    let biz = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            Option<i64>,
        ),
    >(
        "SELECT business_name, address, email, phone, logo_path, tax_label, tax_number, currency, \
         logo_asset_id \
         FROM settings WHERE id = 1",
    )
    .fetch_one(&mut *tx)
    .await?;
    if biz.0.trim().is_empty() {
        return Err(DataError::Other(
            "add your business name in Settings before issuing an invoice".into(),
        ));
    }
    let business_snapshot = serde_json::json!({
        "name": biz.0, "address": biz.1, "email": biz.2, "phone": biz.3,
        "logo_path": biz.4, "tax_label": biz.5, "tax_number": biz.6, "currency": biz.7,
    })
    .to_string();
    let cust = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT name, email, phone, billing_address FROM customer \
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(customer_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| DataError::Other("active customer not found".into()))?;
    let customer_snapshot = serde_json::json!({
        "name": cust.0, "email": cust.1, "phone": cust.2, "address": cust.3,
    })
    .to_string();

    // Decrement stock once for each tracked product line.
    let lines = sqlx::query_as::<_, (Option<i64>, String)>(
        "SELECT item_id, quantity FROM invoice_line WHERE invoice_id = ?",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    for (item_id, qty_str) in lines {
        if let Some(item_id) = item_id {
            let tracked: Option<bool> = sqlx::query_scalar("SELECT tracked FROM item WHERE id = ?")
                .bind(item_id)
                .fetch_optional(&mut *tx)
                .await?;
            if tracked == Some(true) {
                let qty = tracked_qty(&qty_str)?;
                if qty != 0 {
                    items::apply_movement(
                        &mut tx,
                        item_id,
                        -qty,
                        MovementReason::Sale,
                        Some("invoice"),
                        Some(id),
                        "",
                    )
                    .await?;
                }
            }
        }
    }

    let new_status = payment_status(Money::from_minor(total_minor), Money::ZERO).as_db();
    sqlx::query(
        "UPDATE invoice SET status = ?, number = ?, issue_date = date('now','localtime'), \
         issued_at = datetime('now'), business_snapshot = ?, customer_snapshot = ?, \
         business_logo_asset_id = ?, \
         due_date = CASE WHEN due_days IS NULL THEN due_date \
                         ELSE max(COALESCE(due_date, ''), \
                                  date('now', 'localtime', printf('+%d days', due_days))) END \
         WHERE id = ?",
    )
    .bind(new_status)
    .bind(&number)
    .bind(&business_snapshot)
    .bind(&customer_snapshot)
    .bind(biz.8)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Void an issued, **unpaid** invoice: reverse the stock movements it recorded and release the quote
/// or job it came from. The number is kept (never deleted). Invoices with recorded payments cannot
/// be voided (refunds/credit notes are a later feature) — this keeps the cash trail intact.
pub async fn void(db: &Db, id: i64) -> Result<(), DataError> {
    let mut tx = begin_write(db).await?;
    let claimed: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
        "UPDATE invoice SET status = 'void', voided_at = datetime('now'), \
         void_date = date('now', 'localtime') \
         WHERE id = ? AND status IN ('issued', 'part_paid', 'paid') \
           AND COALESCE((SELECT SUM(amount_minor) FROM payment_allocation \
                         WHERE invoice_id = ?), 0) = 0 \
         RETURNING source_quote_id, source_job_id",
    )
    .bind(id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((source_quote_id, source_job_id)) = claimed else {
        return Err(DataError::Other(
            "invoice not found, already void, still a draft, or has recorded payments".into(),
        ));
    };

    // Reverse exactly what issuing recorded, rather than re-deriving it from the items' current
    // settings: an item's tracking can be switched on after the sale, which must not invent stock.
    let sold: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT item_id, SUM(qty_delta) FROM stock_movement \
         WHERE ref_type = 'invoice' AND ref_id = ? GROUP BY item_id HAVING SUM(qty_delta) <> 0",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    for (item_id, delta) in sold {
        items::apply_movement(
            &mut tx,
            item_id,
            -delta,
            MovementReason::Return,
            Some("invoice-void"),
            Some(id),
            "",
        )
        .await?;
    }
    release_sources(&mut tx, id, source_quote_id, source_job_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list(db: &Db) -> Result<Vec<InvoiceRow>, DataError> {
    Ok(
        sqlx::query_as::<_, InvoiceRow>(concat!(invoice_row_select!(), " ORDER BY i.id DESC"))
            .fetch_all(db)
            .await?,
    )
}

pub async fn list_for_customer(db: &Db, customer_id: i64) -> Result<Vec<InvoiceRow>, DataError> {
    Ok(sqlx::query_as::<_, InvoiceRow>(concat!(
        invoice_row_select!(),
        " WHERE i.customer_id = ? ORDER BY i.id DESC"
    ))
    .bind(customer_id)
    .fetch_all(db)
    .await?)
}

pub async fn get_detail(db: &Db, id: i64) -> Result<Option<InvoiceDetail>, DataError> {
    let invoice =
        sqlx::query_as::<_, InvoiceRow>(concat!(invoice_row_select!(), " WHERE i.id = ?"))
            .bind(id)
            .fetch_optional(db)
            .await?;
    let Some(invoice) = invoice else {
        return Ok(None);
    };

    let lines = sqlx::query_as::<_, InvoiceLineRow>(
        "SELECT id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, \
         tax_inclusive, net_minor, tax_minor, gross_minor FROM invoice_line WHERE invoice_id = ? ORDER BY line_order",
    )
    .bind(id)
    .fetch_all(db)
    .await?;

    let amount_paid_minor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0) FROM payment_allocation WHERE invoice_id = ?",
    )
    .bind(id)
    .fetch_one(db)
    .await?;

    let (
        business_snapshot,
        customer_snapshot,
        tax_summary,
        business_logo_data,
        business_logo_format,
    ) = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            String,
            Option<Vec<u8>>,
            Option<String>,
        ),
    >(
        "SELECT i.business_snapshot, i.customer_snapshot, i.tax_summary, a.data, a.format \
             FROM invoice i LEFT JOIN business_logo_asset a \
             ON a.id = i.business_logo_asset_id WHERE i.id = ?",
    )
    .bind(id)
    .fetch_one(db)
    .await?;

    Ok(Some(InvoiceDetail {
        invoice,
        lines,
        amount_paid_minor,
        business_snapshot,
        customer_snapshot,
        tax_summary,
        business_logo_data,
        business_logo_format,
    }))
}

/// Status helper exposed for tests / callers (uses the domain rule).
pub fn status_for(total_minor: i64, paid_minor: i64) -> InvoiceStatus {
    payment_status(
        Money::from_minor(total_minor),
        Money::from_minor(paid_minor),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::{customers, items, settings};

    async fn seed_customer(db: &Db) -> i64 {
        let mut business = settings::get(db).await.unwrap();
        business.business_name = "Test business".into();
        settings::update(db, &business).await.unwrap();
        customers::create(
            db,
            &customers::CustomerInput {
                name: "Acme".into(),
                email: "a@b.c".into(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await
        .unwrap()
    }

    fn line(item_id: Option<i64>, qty: &str, price: i64, bp: i64) -> LineInput {
        LineInput {
            item_id,
            description: "Work".into(),
            quantity: qty.into(),
            unit_price_minor: price,
            tax_rate_name: "VAT 20%".into(),
            tax_rate_bp: bp,
            tax_inclusive: false,
        }
    }

    #[sqlx::test]
    async fn issue_assigns_number_freezes_and_decrements_stock_once(
        pool: Db,
    ) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let mut original_settings = settings::get(&pool).await?;
        original_settings.business_name = "Original business".into();
        original_settings.currency = "AUD".into();
        original_settings.tax_label = "GST".into();
        original_settings.tax_number = "ABN-ORIGINAL".into();
        settings::update(&pool, &original_settings).await?;
        settings::set_logo_asset(
            &pool,
            Some("/test/original-logo.png"),
            Some((b"original-logo", "png")),
        )
        .await?;
        let item = items::create(
            &pool,
            &items::ItemInput {
                kind: "product".into(),
                name: "Widget".into(),
                sku: "W".into(),
                unit: "each".into(),
                default_price_minor: 1000,
                default_tax_rate_id: None,
                tracked: true,
                reorder_point: None,
            },
        )
        .await?;
        items::adjust_stock(&pool, item, 10, "initial").await?;

        let inv = create_draft(
            &pool,
            customer,
            &[line(Some(item), "3", 1000, 2000)],
            None,
            "",
        )
        .await?;
        issue(&pool, inv).await?;

        // Later edits must not rewrite the identity or currency of the issued invoice.
        let mut changed_settings = settings::get(&pool).await?;
        changed_settings.business_name = "Renamed business".into();
        changed_settings.tax_number = "NEW-NUMBER".into();
        settings::update(&pool, &changed_settings).await?;
        // The currency is locked once invoices are issued; older builds allowed changing it.
        assert!(settings::update(
            &pool,
            &settings::Settings {
                currency: "USD".into(),
                ..changed_settings
            }
        )
        .await
        .is_err());
        sqlx::query("UPDATE settings SET currency = 'USD' WHERE id = 1")
            .execute(&pool)
            .await?;
        settings::set_logo_asset(
            &pool,
            Some("/test/new-logo.jpg"),
            Some((b"new-logo", "jpg")),
        )
        .await?;
        customers::update(
            &pool,
            customer,
            &customers::CustomerInput {
                name: "Renamed customer".into(),
                email: "new@example.com".into(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await?;

        let detail = get_detail(&pool, inv).await?.unwrap();
        assert_eq!(detail.invoice.status, "issued");
        assert_eq!(detail.invoice.number.as_deref(), Some("INV-0001"));
        // 3 × $10.00 = $30.00 net, 20% = $6.00 tax, $36.00 total
        assert_eq!(detail.invoice.subtotal_minor, 3000);
        assert_eq!(detail.invoice.tax_minor, 600);
        assert_eq!(detail.invoice.total_minor, 3600);
        // stock decremented exactly once: 10 - 3 = 7
        assert_eq!(items::get(&pool, item).await?.unwrap().qty_on_hand, 7);
        let business: serde_json::Value =
            serde_json::from_str(detail.business_snapshot.as_deref().unwrap()).unwrap();
        let customer_snapshot: serde_json::Value =
            serde_json::from_str(detail.customer_snapshot.as_deref().unwrap()).unwrap();
        assert_eq!(business["name"], "Original business");
        assert_eq!(business["currency"], "AUD");
        assert_eq!(business["tax_number"], "ABN-ORIGINAL");
        assert_eq!(customer_snapshot["name"], "Acme");
        assert_eq!(
            detail.business_logo_data.as_deref(),
            Some(b"original-logo".as_slice())
        );
        assert_eq!(detail.business_logo_format.as_deref(), Some("png"));
        assert!(detail.tax_summary.contains("VAT 20%"));
        assert!(settings::get(&pool).await?.invoice_next_seq == 2);

        // re-issuing fails (immutable)
        assert!(issue(&pool, inv).await.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn update_and_delete_draft_only(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let inv = create_draft(&pool, customer, &[line(None, "1", 1000, 0)], None, "v1").await?;

        // Edit the draft: new lines + due date + notes, totals recomputed.
        update_draft(
            &pool,
            inv,
            customer,
            &[line(None, "2", 2500, 2000)],
            Some("2026-07-01"),
            "v2",
        )
        .await?;
        let d = get_detail(&pool, inv).await?.unwrap();
        assert_eq!(d.lines.len(), 1);
        assert_eq!(d.invoice.subtotal_minor, 5000); // 2 × $25.00
        assert_eq!(d.invoice.tax_minor, 1000); // 20%
        assert_eq!(d.invoice.due_date.as_deref(), Some("2026-07-01"));
        assert_eq!(d.invoice.notes, "v2");

        // Empty lines rejected.
        assert!(update_draft(&pool, inv, customer, &[], None, "")
            .await
            .is_err());

        // Once issued: immutable — no edit, no delete.
        issue(&pool, inv).await?;
        assert!(
            update_draft(&pool, inv, customer, &[line(None, "1", 1, 0)], None, "")
                .await
                .is_err()
        );
        assert!(delete_draft(&pool, inv).await.is_err());

        // A fresh draft deletes cleanly (lines too).
        let doomed = create_draft(&pool, customer, &[line(None, "1", 500, 0)], None, "").await?;
        delete_draft(&pool, doomed).await?;
        assert!(get_detail(&pool, doomed).await?.is_none());
        let orphan_lines: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM invoice_line WHERE invoice_id = ?")
                .bind(doomed)
                .fetch_one(&pool)
                .await?;
        assert_eq!(orphan_lines, 0);
        Ok(())
    }

    #[sqlx::test]
    async fn invalid_document_lines_return_errors_without_partial_rows(
        pool: Db,
    ) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let mut cases = vec![
            line(None, "0", 100, 0),
            line(None, "-1", 100, 0),
            line(None, "1", -1, 0),
            line(None, "1", 100, -1),
            line(None, "2", i64::MAX, 0),
            line(Some(99_999), "1", 100, 0),
        ];
        let mut blank = line(None, "1", 100, 0);
        blank.description = "   ".into();
        cases.push(blank);
        let mut oversized_description = line(None, "1", 100, 0);
        oversized_description.description = "x".repeat(2_001);
        cases.push(oversized_description);
        let mut oversized_tax_name = line(None, "1", 100, 0);
        oversized_tax_name.tax_rate_name = "x".repeat(101);
        cases.push(oversized_tax_name);
        cases.push(line(None, "1000000001", 100, 0));
        cases.push(line(None, &"1".repeat(101), 100, 0));

        for invalid in cases {
            assert!(
                create_draft(&pool, customer, &[invalid], None, "")
                    .await
                    .is_err(),
                "invalid document input must be rejected"
            );
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 0, "failed validation must not leave draft rows");
        Ok(())
    }

    #[sqlx::test]
    async fn discount_lines_reduce_the_total_but_never_below_zero(
        pool: Db,
    ) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let mut discount = line(None, "1", -2500, 2000);
        discount.description = "Loyalty discount".into();
        let inv = create_draft(
            &pool,
            customer,
            &[line(None, "1", 10000, 2000), discount.clone()],
            None,
            "",
        )
        .await?;
        let detail = get_detail(&pool, inv).await?.unwrap();
        assert_eq!(detail.invoice.subtotal_minor, 7500);
        assert_eq!(detail.invoice.tax_minor, 1500);
        assert_eq!(detail.invoice.total_minor, 9000);

        let mut too_big = discount;
        too_big.unit_price_minor = -20000;
        assert!(
            create_draft(
                &pool,
                customer,
                &[line(None, "1", 10000, 2000), too_big],
                None,
                ""
            )
            .await
            .is_err(),
            "a document total below zero is rejected"
        );
        Ok(())
    }

    #[sqlx::test]
    async fn void_restores_stock_and_keeps_number(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let item = items::create(
            &pool,
            &items::ItemInput {
                kind: "product".into(),
                name: "Bolt".into(),
                sku: "B".into(),
                unit: "each".into(),
                default_price_minor: 500,
                default_tax_rate_id: None,
                tracked: true,
                reorder_point: None,
            },
        )
        .await?;
        items::adjust_stock(&pool, item, 5, "initial").await?;
        let inv = create_draft(&pool, customer, &[line(Some(item), "2", 500, 0)], None, "").await?;
        issue(&pool, inv).await?;
        assert_eq!(items::get(&pool, item).await?.unwrap().qty_on_hand, 3);

        void(&pool, inv).await?;
        let detail = get_detail(&pool, inv).await?.unwrap();
        assert_eq!(detail.invoice.status, "void");
        assert_eq!(detail.invoice.number.as_deref(), Some("INV-0001")); // number kept
        assert_eq!(items::get(&pool, item).await?.unwrap().qty_on_hand, 5); // restored
        Ok(())
    }

    #[sqlx::test]
    async fn fractional_tracked_quantity_is_rejected_at_issue(pool: Db) -> Result<(), DataError> {
        let customer = seed_customer(&pool).await;
        let item = items::create(
            &pool,
            &items::ItemInput {
                kind: "product".into(),
                name: "Widget".into(),
                sku: "W".into(),
                unit: "each".into(),
                default_price_minor: 1000,
                default_tax_rate_id: None,
                tracked: true,
                reorder_point: None,
            },
        )
        .await?;
        items::adjust_stock(&pool, item, 10, "init").await?;
        let inv = create_draft(
            &pool,
            customer,
            &[line(Some(item), "2.5", 1000, 0)],
            None,
            "",
        )
        .await?;
        assert!(issue(&pool, inv).await.is_err()); // fractional qty on a tracked product
        assert_eq!(items::get(&pool, item).await?.unwrap().qty_on_hand, 10); // tx rolled back
        Ok(())
    }
}
