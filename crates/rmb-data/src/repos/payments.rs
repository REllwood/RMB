//! Payments + allocations. Recording a payment allocates against the outstanding balance, then derives
//! and stores the invoice's payment status (issued → part_paid → paid).

use rmb_domain::status::{payment_status, InvoiceStatus};
use rmb_domain::Money;
use serde::Serialize;
use sqlx::FromRow;

use crate::db::{begin_write, Db};
use crate::error::DataError;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Payment {
    pub id: i64,
    pub date: String,
    pub amount_minor: i64,
    pub method: String,
    pub reference: String,
}

/// Check a payment-driven status change against the domain's transition rules.
fn check_transition(current: &str, next: InvoiceStatus) -> Result<(), DataError> {
    let from = InvoiceStatus::from_db(current)
        .ok_or_else(|| DataError::Other(format!("invoice has an unknown status '{current}'")))?;
    if from != next && !from.can_transition_to(next) {
        return Err(DataError::Other(format!(
            "a {current} invoice can't become {}",
            next.as_db()
        )));
    }
    Ok(())
}

/// Record a payment against an invoice. Overpayments are rejected because v1 has no customer-credit
/// ledger; silently keeping an unallocated remainder would make receipts and cash reports disagree.
/// Returns the allocated amount and recomputes the invoice status.
pub async fn record_payment(
    db: &Db,
    invoice_id: i64,
    amount_minor: i64,
    method: &str,
    reference: &str,
) -> Result<i64, DataError> {
    if amount_minor <= 0 {
        return Err(DataError::Other("payment amount must be positive".into()));
    }
    let method = method.trim();
    let reference = reference.trim();
    if method.is_empty() {
        return Err(DataError::Other("payment method is required".into()));
    }
    if method.chars().count() > 100 || reference.chars().count() > 500 {
        return Err(DataError::Other(
            "payment method or reference is too long".into(),
        ));
    }
    let mut tx = begin_write(db).await?;

    // A harmless update is deliberately the first operation: it takes SQLite's write lock before
    // the outstanding balance is read, serialising simultaneous payment attempts for this file.
    let (status, total, customer_id) = sqlx::query_as::<_, (String, i64, i64)>(
        "UPDATE invoice SET status = status WHERE id = ? \
         RETURNING status, total_minor, customer_id",
    )
    .bind(invoice_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| DataError::Other("invoice not found".into()))?;
    if status == "draft" || status == "void" {
        return Err(DataError::Other("can only pay an issued invoice".into()));
    }

    let allocated: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0) FROM payment_allocation WHERE invoice_id = ?",
    )
    .bind(invoice_id)
    .fetch_one(&mut *tx)
    .await?;
    let outstanding = (total - allocated).max(0);
    if outstanding <= 0 {
        return Err(DataError::Other(
            "invoice has no outstanding balance".into(),
        ));
    }
    if amount_minor > outstanding {
        return Err(DataError::Other(format!(
            "payment exceeds the outstanding balance (maximum is {outstanding} minor units)"
        )));
    }
    let alloc = amount_minor;

    // `localtime`: the payment date is a business-facing date on the user's machine, not a UTC
    // audit stamp — UTC would show "yesterday" for morning payments east of Greenwich.
    let payment_id = sqlx::query(
        "INSERT INTO payment (customer_id, date, amount_minor, method, reference) \
         VALUES (?, datetime('now','localtime'), ?, ?, ?)",
    )
    .bind(customer_id)
    .bind(amount_minor)
    .bind(method)
    .bind(reference)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    sqlx::query(
        "INSERT INTO payment_allocation (payment_id, invoice_id, amount_minor) VALUES (?, ?, ?)",
    )
    .bind(payment_id)
    .bind(invoice_id)
    .bind(alloc)
    .execute(&mut *tx)
    .await?;

    // Derive status from the authoritative SUM *after* inserting (avoids stale-read drift under
    // concurrent payments — SQLite serializes write transactions).
    let total_allocated: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0) FROM payment_allocation WHERE invoice_id = ?",
    )
    .bind(invoice_id)
    .fetch_one(&mut *tx)
    .await?;
    if total_allocated > total {
        return Err(DataError::Other(
            "payment allocations exceed the invoice total".into(),
        ));
    }
    let new_status = payment_status(Money::from_minor(total), Money::from_minor(total_allocated));
    check_transition(&status, new_status)?;
    sqlx::query("UPDATE invoice SET status = ? WHERE id = ? AND status != 'void'")
        .bind(new_status.as_db())
        .bind(invoice_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(alloc)
}

/// Delete a mis-entered payment (and its allocations), re-deriving each affected invoice's
/// status from the remaining allocations. This is the v1 correction path for "paid the wrong
/// invoice" — proper credit notes/refunds are a later module.
pub async fn delete_payment(db: &Db, payment_id: i64) -> Result<(), DataError> {
    let mut tx = begin_write(db).await?;

    let invoice_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT invoice_id FROM payment_allocation WHERE payment_id = ?",
    )
    .bind(payment_id)
    .fetch_all(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM payment_allocation WHERE payment_id = ?")
        .bind(payment_id)
        .execute(&mut *tx)
        .await?;
    let deleted = sqlx::query("DELETE FROM payment WHERE id = ?")
        .bind(payment_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(DataError::Other("payment not found".into()));
    }

    for invoice_id in invoice_ids {
        let (total, status) = sqlx::query_as::<_, (i64, String)>(
            "SELECT total_minor, status FROM invoice WHERE id = ?",
        )
        .bind(invoice_id)
        .fetch_one(&mut *tx)
        .await?;
        let paid: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_minor), 0) FROM payment_allocation WHERE invoice_id = ?",
        )
        .bind(invoice_id)
        .fetch_one(&mut *tx)
        .await?;
        // Only payment-derived statuses move; draft/void are explicit states left untouched.
        if matches!(status.as_str(), "issued" | "part_paid" | "paid") {
            let new_status = payment_status(Money::from_minor(total), Money::from_minor(paid));
            check_transition(&status, new_status)?;
            sqlx::query("UPDATE invoice SET status = ? WHERE id = ?")
                .bind(new_status.as_db())
                .bind(invoice_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

pub async fn list_for_invoice(db: &Db, invoice_id: i64) -> Result<Vec<Payment>, DataError> {
    Ok(sqlx::query_as::<_, Payment>(
        "SELECT p.id, p.date, a.amount_minor, p.method, p.reference \
         FROM payment p JOIN payment_allocation a ON a.payment_id = p.id \
         WHERE a.invoice_id = ? ORDER BY p.id DESC",
    )
    .bind(invoice_id)
    .fetch_all(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::{customers, invoices, settings};

    async fn issued_invoice(db: &Db, total_each: i64) -> i64 {
        let mut business = settings::get(db).await.unwrap();
        business.business_name = "Test business".into();
        settings::update(db, &business).await.unwrap();
        let customer = customers::create(
            db,
            &customers::CustomerInput {
                name: "C".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await
        .unwrap();
        let inv = invoices::create_draft(
            db,
            customer,
            &[invoices::LineInput {
                item_id: None,
                description: "Service".into(),
                quantity: "1".into(),
                unit_price_minor: total_each,
                tax_rate_name: "No Tax".into(),
                tax_rate_bp: 0,
                tax_inclusive: false,
            }],
            None,
            "",
        )
        .await
        .unwrap();
        invoices::issue(db, inv).await.unwrap();
        inv
    }

    #[sqlx::test]
    async fn partial_then_full_payment_flows_status(pool: Db) -> Result<(), DataError> {
        let inv = issued_invoice(&pool, 10000).await; // $100.00

        record_payment(&pool, inv, 4000, "cash", "").await?;
        assert_eq!(
            invoices::get_detail(&pool, inv)
                .await?
                .unwrap()
                .invoice
                .status,
            "part_paid"
        );

        record_payment(&pool, inv, 6000, "card", "").await?;
        let detail = invoices::get_detail(&pool, inv).await?.unwrap();
        assert_eq!(detail.invoice.status, "paid");
        assert_eq!(detail.amount_paid_minor, 10000);
        assert_eq!(list_for_invoice(&pool, inv).await?.len(), 2);
        Ok(())
    }

    #[sqlx::test]
    async fn overpayment_is_rejected_without_recording_cash(pool: Db) -> Result<(), DataError> {
        let inv = issued_invoice(&pool, 5000).await;
        assert!(record_payment(&pool, inv, 9999, "cash", "").await.is_err());
        let detail = invoices::get_detail(&pool, inv).await?.unwrap();
        assert_eq!(detail.invoice.status, "issued");
        assert_eq!(detail.amount_paid_minor, 0);
        assert!(list_for_invoice(&pool, inv).await?.is_empty());
        Ok(())
    }

    #[sqlx::test]
    async fn delete_payment_rewinds_status(pool: Db) -> Result<(), DataError> {
        let inv = issued_invoice(&pool, 10000).await;
        let status = |pool: Db| async move {
            invoices::get_detail(&pool, inv)
                .await
                .unwrap()
                .unwrap()
                .invoice
                .status
        };

        record_payment(&pool, inv, 4000, "cash", "").await?;
        record_payment(&pool, inv, 6000, "card", "").await?;
        assert_eq!(status(pool.clone()).await, "paid");

        let payments = list_for_invoice(&pool, inv).await?;
        assert_eq!(payments.len(), 2);

        // Delete the $60.00 card payment → back to part_paid.
        let card = payments.iter().find(|p| p.method == "card").unwrap().id;
        delete_payment(&pool, card).await?;
        assert_eq!(status(pool.clone()).await, "part_paid");
        assert_eq!(
            invoices::get_detail(&pool, inv)
                .await?
                .unwrap()
                .amount_paid_minor,
            4000
        );

        // Delete the remaining payment → back to issued; unknown id errors.
        let cash = list_for_invoice(&pool, inv).await?[0].id;
        delete_payment(&pool, cash).await?;
        assert_eq!(status(pool.clone()).await, "issued");
        assert!(delete_payment(&pool, 99999).await.is_err());
        Ok(())
    }
}
