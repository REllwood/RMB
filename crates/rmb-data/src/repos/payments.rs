//! Payments + allocations. Recording a payment allocates (clamped to outstanding), then derives
//! and stores the invoice's payment status (issued → part_paid → paid).

use rmb_domain::status::payment_status;
use rmb_domain::Money;
use serde::Serialize;
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Payment {
    pub id: i64,
    pub date: String,
    pub amount_minor: i64,
    pub method: String,
    pub reference: String,
}

/// Record a payment against an invoice. The allocated amount is clamped to the outstanding
/// balance (overpayment is recorded on the payment but not over-allocated). Returns the amount
/// actually allocated. Recomputes + stores the invoice status.
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
    let mut tx = db.begin().await?;

    let (status, total, customer_id) = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT status, total_minor, customer_id FROM invoice WHERE id = ?",
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
    let alloc = amount_minor.min(outstanding); // clamp overpayment; always > 0 here

    let payment_id = sqlx::query(
        "INSERT INTO payment (customer_id, amount_minor, method, reference) VALUES (?, ?, ?, ?)",
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
    let new_status = payment_status(Money::from_minor(total), Money::from_minor(total_allocated));
    sqlx::query("UPDATE invoice SET status = ? WHERE id = ? AND status != 'void'")
        .bind(new_status.as_db())
        .bind(invoice_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(alloc)
}

pub async fn list_for_invoice(db: &Db, invoice_id: i64) -> Result<Vec<Payment>, DataError> {
    Ok(sqlx::query_as::<_, Payment>(
        "SELECT p.id, p.date, p.amount_minor, p.method, p.reference \
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
    use crate::repos::{customers, invoices};

    async fn issued_invoice(db: &Db, total_each: i64) -> i64 {
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
    async fn overpayment_is_clamped(pool: Db) -> Result<(), DataError> {
        let inv = issued_invoice(&pool, 5000).await;
        let allocated = record_payment(&pool, inv, 9999, "cash", "").await?;
        assert_eq!(allocated, 5000); // clamped to outstanding
        assert_eq!(
            invoices::get_detail(&pool, inv)
                .await?
                .unwrap()
                .invoice
                .status,
            "paid"
        );
        Ok(())
    }
}
