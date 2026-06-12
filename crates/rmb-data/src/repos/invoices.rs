//! Invoice repository.
//!
//! Drafts are editable and store computed line/total breakdowns. **Issuing** an invoice, in one
//! transaction: assigns the next gapless number, freezes business + customer snapshots, decrements
//! tracked-product stock exactly once, and flips status to `issued`. Issued invoices are immutable —
//! correct via **void** (reverses stock; allowed only while the invoice is unpaid).

use std::str::FromStr;

use rmb_domain::document::{total_lines, DocumentLine};
use rmb_domain::inventory::MovementReason;
use rmb_domain::numbering::format_number;
use rmb_domain::status::{payment_status, InvoiceStatus};
use rmb_domain::tax::{line_tax, TaxRate};
use rmb_domain::Money;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;
use crate::repos::items;

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
}

pub(crate) fn to_doc_line(l: &LineInput) -> Result<DocumentLine, DataError> {
    let qty = Decimal::from_str(l.quantity.trim())
        .map_err(|e| DataError::Other(format!("invalid quantity '{}': {e}", l.quantity)))?;
    Ok(DocumentLine::new(
        l.description.clone(),
        qty,
        Money::from_minor(l.unit_price_minor),
        TaxRate::new(l.tax_rate_name.clone(), l.tax_rate_bp, l.tax_inclusive),
    ))
}

/// Quantity for a tracked-product stock movement: must be a **whole, non-negative** number.
/// Tracked stock is counted in whole units, so fractional/negative quantities are rejected
/// rather than silently rounded (which would desync stock from what was billed).
fn tracked_qty(qty_str: &str) -> Result<i64, DataError> {
    let qty = Decimal::from_str(qty_str.trim())
        .map_err(|e| DataError::Other(format!("invalid quantity '{qty_str}': {e}")))?;
    if qty < Decimal::ZERO || qty.fract() != Decimal::ZERO {
        return Err(DataError::Other(format!(
            "a tracked product line needs a whole, non-negative quantity (got {qty})"
        )));
    }
    Ok(qty.to_i64().unwrap_or(0))
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
    let doc_lines = lines
        .iter()
        .map(to_doc_line)
        .collect::<Result<Vec<_>, _>>()?;
    let totals = total_lines(&doc_lines);
    let tax_summary = serde_json::to_string(&totals.tax_summary).unwrap_or_else(|_| "[]".into());

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
        .bind(&l.description)
        .bind(&l.quantity)
        .bind(l.unit_price_minor)
        .bind(&l.tax_rate_name)
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
    let mut tx = db.begin().await?;
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
    let doc_lines = lines
        .iter()
        .map(to_doc_line)
        .collect::<Result<Vec<_>, _>>()?;
    let totals = total_lines(&doc_lines);
    let tax_summary = serde_json::to_string(&totals.tax_summary).unwrap_or_else(|_| "[]".into());

    let mut tx = db.begin().await?;
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM invoice WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    match status.as_deref() {
        None => return Err(DataError::Other("invoice not found".into())),
        Some("draft") => {}
        Some(_) => {
            return Err(DataError::Other(
                "only draft invoices can be edited (void + reissue to correct an issued one)"
                    .into(),
            ))
        }
    }

    sqlx::query(
        "UPDATE invoice SET customer_id = ?, due_date = ?, notes = ?, subtotal_minor = ?, \
         tax_minor = ?, total_minor = ?, tax_summary = ? WHERE id = ?",
    )
    .bind(customer_id)
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

/// Hard-delete a **draft** invoice (and its lines). Drafts have no number, no stock effect, and
/// no payments, so deletion is safe; issued invoices are voided, never deleted.
pub async fn delete_draft(db: &Db, id: i64) -> Result<(), DataError> {
    let mut tx = db.begin().await?;
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM invoice WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    match status.as_deref() {
        None => return Err(DataError::Other("invoice not found".into())),
        Some("draft") => {}
        Some(_) => {
            return Err(DataError::Other(
                "only draft invoices can be deleted (issued invoices are voided instead)".into(),
            ))
        }
    }
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

/// Issue a draft: assign number, freeze snapshots, decrement stock once, set status = issued.
pub async fn issue(db: &Db, id: i64) -> Result<(), DataError> {
    let mut tx = db.begin().await?;

    let row = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT status, customer_id, total_minor FROM invoice WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (status, customer_id, total_minor) =
        row.ok_or_else(|| DataError::Other("invoice not found".into()))?;
    if status != "draft" {
        return Err(DataError::Other("only draft invoices can be issued".into()));
    }
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
    let number = format_number(&prefix, seq, pad.max(0) as usize);
    sqlx::query("UPDATE settings SET invoice_next_seq = invoice_next_seq + 1 WHERE id = 1")
        .execute(&mut *tx)
        .await?;

    // Freeze business + customer snapshots.
    let biz = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, String)>(
        "SELECT business_name, address, email, phone, logo_path, tax_label, tax_number FROM settings WHERE id = 1",
    )
    .fetch_one(&mut *tx)
    .await?;
    let business_snapshot = serde_json::json!({
        "name": biz.0, "address": biz.1, "email": biz.2, "phone": biz.3,
        "logo_path": biz.4, "tax_label": biz.5, "tax_number": biz.6,
    })
    .to_string();
    let cust = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT name, email, phone, billing_address FROM customer WHERE id = ?",
    )
    .bind(customer_id)
    .fetch_one(&mut *tx)
    .await?;
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
         issued_at = datetime('now'), business_snapshot = ?, customer_snapshot = ? WHERE id = ?",
    )
    .bind(new_status)
    .bind(&number)
    .bind(&business_snapshot)
    .bind(&customer_snapshot)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Void an issued, **unpaid** invoice: reverse stock. The number is kept (never deleted). Invoices
/// with recorded payments cannot be voided (refunds/credit notes are a later feature) — this keeps
/// the cash trail intact.
pub async fn void(db: &Db, id: i64) -> Result<(), DataError> {
    let mut tx = db.begin().await?;
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM invoice WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let status = status.ok_or_else(|| DataError::Other("invoice not found".into()))?;
    if status == "draft" || status == "void" {
        return Err(DataError::Other(
            "only issued invoices can be voided".into(),
        ));
    }
    let allocated: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0) FROM payment_allocation WHERE invoice_id = ?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if allocated > 0 {
        return Err(DataError::Other(
            "cannot void an invoice with recorded payments (refunds/credit notes are a later feature)".into(),
        ));
    }

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
                        qty,
                        MovementReason::Return,
                        Some("invoice-void"),
                        Some(id),
                        "",
                    )
                    .await?;
                }
            }
        }
    }
    sqlx::query("UPDATE invoice SET status = 'void', voided_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list(db: &Db) -> Result<Vec<InvoiceRow>, DataError> {
    Ok(sqlx::query_as::<_, InvoiceRow>(
        "SELECT id, customer_id, number, status, issue_date, due_date, subtotal_minor, tax_minor, \
         total_minor, notes, created_at FROM invoice ORDER BY id DESC",
    )
    .fetch_all(db)
    .await?)
}

pub async fn list_for_customer(db: &Db, customer_id: i64) -> Result<Vec<InvoiceRow>, DataError> {
    Ok(sqlx::query_as::<_, InvoiceRow>(
        "SELECT id, customer_id, number, status, issue_date, due_date, subtotal_minor, tax_minor, \
         total_minor, notes, created_at FROM invoice WHERE customer_id = ? ORDER BY id DESC",
    )
    .bind(customer_id)
    .fetch_all(db)
    .await?)
}

pub async fn get_detail(db: &Db, id: i64) -> Result<Option<InvoiceDetail>, DataError> {
    let invoice = sqlx::query_as::<_, InvoiceRow>(
        "SELECT id, customer_id, number, status, issue_date, due_date, subtotal_minor, tax_minor, \
         total_minor, notes, created_at FROM invoice WHERE id = ?",
    )
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

    Ok(Some(InvoiceDetail {
        invoice,
        lines,
        amount_paid_minor,
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

        let detail = get_detail(&pool, inv).await?.unwrap();
        assert_eq!(detail.invoice.status, "issued");
        assert_eq!(detail.invoice.number.as_deref(), Some("INV-0001"));
        // 3 × $10.00 = $30.00 net, 20% = $6.00 tax, $36.00 total
        assert_eq!(detail.invoice.subtotal_minor, 3000);
        assert_eq!(detail.invoice.tax_minor, 600);
        assert_eq!(detail.invoice.total_minor, 3600);
        // stock decremented exactly once: 10 - 3 = 7
        assert_eq!(items::get(&pool, item).await?.unwrap().qty_on_hand, 7);
        // business/customer snapshots frozen
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
