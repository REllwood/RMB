//! Reporting aggregates. All revenue reports are **accrual basis** — grouped by invoice issue
//! date, counting issued / part-paid / paid invoices (drafts and voids excluded). The payments
//! export is cash basis (by payment date). Optional date bounds are inclusive `YYYY-MM-DD`;
//! `None` means unbounded (COALESCE sentinels keep the SQL static for sqlx's safety check).

use serde::Serialize;
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;
use crate::validation::valid_business_date;

fn validate_range(from: Option<&str>, to: Option<&str>) -> Result<(), DataError> {
    if from.is_some_and(|date| !valid_business_date(date))
        || to.is_some_and(|date| !valid_business_date(date))
    {
        return Err(DataError::Other(
            "report dates must use valid YYYY-MM-DD values".into(),
        ));
    }
    if let (Some(from), Some(to)) = (from, to) {
        if from > to {
            return Err(DataError::Other(
                "report start date cannot be after the end date".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TaxSummaryRow {
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub net_minor: i64,
    pub tax_minor: i64,
    pub gross_minor: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MonthlySalesRow {
    pub month: String, // YYYY-MM
    pub invoice_count: i64,
    pub net_minor: i64,
    pub tax_minor: i64,
    pub gross_minor: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CustomerSalesRow {
    pub customer_id: i64,
    pub name: String,
    pub invoice_count: i64,
    pub gross_minor: i64,
    pub paid_minor: i64,
}

/// Tax collected per rate (the BAS / VAT-return view), from line-level snapshots.
pub async fn tax_summary(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<TaxSummaryRow>, DataError> {
    validate_range(from, to)?;
    Ok(sqlx::query_as::<_, TaxSummaryRow>(
        "SELECT l.tax_rate_name, l.tax_rate_bp, \
         COALESCE(SUM(l.net_minor), 0) AS net_minor, \
         COALESCE(SUM(l.tax_minor), 0) AS tax_minor, \
         COALESCE(SUM(l.gross_minor), 0) AS gross_minor \
         FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id \
         WHERE i.status IN ('issued', 'part_paid', 'paid') AND i.issue_date IS NOT NULL \
         AND i.issue_date >= COALESCE(?, '0000-00-00') AND i.issue_date <= COALESCE(?, '9999-12-31') \
         GROUP BY l.tax_rate_name, l.tax_rate_bp \
         ORDER BY l.tax_rate_bp DESC, l.tax_rate_name",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?)
}

/// Sales totals per calendar month of issue.
pub async fn sales_by_month(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<MonthlySalesRow>, DataError> {
    validate_range(from, to)?;
    Ok(sqlx::query_as::<_, MonthlySalesRow>(
        "SELECT substr(i.issue_date, 1, 7) AS month, COUNT(*) AS invoice_count, \
         COALESCE(SUM(i.subtotal_minor), 0) AS net_minor, \
         COALESCE(SUM(i.tax_minor), 0) AS tax_minor, \
         COALESCE(SUM(i.total_minor), 0) AS gross_minor \
         FROM invoice i \
         WHERE i.status IN ('issued', 'part_paid', 'paid') AND i.issue_date IS NOT NULL \
         AND i.issue_date >= COALESCE(?, '0000-00-00') AND i.issue_date <= COALESCE(?, '9999-12-31') \
         GROUP BY month ORDER BY month DESC",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?)
}

/// Sales + collected cash per customer, biggest first.
pub async fn sales_by_customer(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<CustomerSalesRow>, DataError> {
    validate_range(from, to)?;
    Ok(sqlx::query_as::<_, CustomerSalesRow>(
        "SELECT i.customer_id, COALESCE(c.name, '(removed)') AS name, COUNT(*) AS invoice_count, \
         COALESCE(SUM(i.total_minor), 0) AS gross_minor, \
         COALESCE(SUM(COALESCE(a.paid, 0)), 0) AS paid_minor \
         FROM invoice i \
         LEFT JOIN customer c ON c.id = i.customer_id \
         LEFT JOIN (SELECT invoice_id, SUM(amount_minor) AS paid FROM payment_allocation GROUP BY invoice_id) a \
         ON a.invoice_id = i.id \
         WHERE i.status IN ('issued', 'part_paid', 'paid') AND i.issue_date IS NOT NULL \
         AND i.issue_date >= COALESCE(?, '0000-00-00') AND i.issue_date <= COALESCE(?, '9999-12-31') \
         GROUP BY i.customer_id, name ORDER BY gross_minor DESC",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?)
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InvoiceExportRow {
    pub id: i64,
    pub number: Option<String>,
    pub customer: String,
    pub status: String,
    pub issue_date: Option<String>,
    pub due_date: Option<String>,
    pub subtotal_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub paid_minor: i64,
}

/// Flat invoice rows for CSV export. With a date range, only issued documents in range; with no
/// range, everything including drafts (a full data dump).
pub async fn invoice_export_rows(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<InvoiceExportRow>, DataError> {
    validate_range(from, to)?;
    let base_select = if from.is_none() && to.is_none() {
        "SELECT i.id, i.number, COALESCE(c.name, '(removed)') AS customer, i.status, \
         i.issue_date, i.due_date, i.subtotal_minor, i.tax_minor, i.total_minor, \
         COALESCE(a.paid, 0) AS paid_minor \
         FROM invoice i LEFT JOIN customer c ON c.id = i.customer_id \
         LEFT JOIN (SELECT invoice_id, SUM(amount_minor) AS paid FROM payment_allocation GROUP BY invoice_id) a \
         ON a.invoice_id = i.id ORDER BY i.id"
    } else {
        "SELECT i.id, i.number, COALESCE(c.name, '(removed)') AS customer, i.status, \
         i.issue_date, i.due_date, i.subtotal_minor, i.tax_minor, i.total_minor, \
         COALESCE(a.paid, 0) AS paid_minor \
         FROM invoice i LEFT JOIN customer c ON c.id = i.customer_id \
         LEFT JOIN (SELECT invoice_id, SUM(amount_minor) AS paid FROM payment_allocation GROUP BY invoice_id) a \
         ON a.invoice_id = i.id \
         WHERE i.issue_date IS NOT NULL \
         AND i.issue_date >= COALESCE(?, '0000-00-00') AND i.issue_date <= COALESCE(?, '9999-12-31') \
         ORDER BY i.id"
    };
    let mut q = sqlx::query_as::<_, InvoiceExportRow>(base_select);
    if !(from.is_none() && to.is_none()) {
        q = q.bind(from).bind(to);
    }
    Ok(q.fetch_all(db).await?)
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PaymentExportRow {
    pub date: String,
    pub amount_minor: i64,
    pub method: String,
    pub reference: String,
    pub invoice_number: Option<String>,
    pub customer: String,
}

/// Flat payment rows (cash basis, by payment date) for CSV export.
pub async fn payment_export_rows(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<PaymentExportRow>, DataError> {
    validate_range(from, to)?;
    Ok(sqlx::query_as::<_, PaymentExportRow>(
        "SELECT p.date, a.amount_minor, p.method, p.reference, i.number AS invoice_number, \
         COALESCE(c.name, '(removed)') AS customer \
         FROM payment p \
         JOIN payment_allocation a ON a.payment_id = p.id \
         JOIN invoice i ON i.id = a.invoice_id \
         LEFT JOIN customer c ON c.id = i.customer_id \
         WHERE substr(p.date, 1, 10) >= COALESCE(?, '0000-00-00') \
         AND substr(p.date, 1, 10) <= COALESCE(?, '9999-12-31') \
         ORDER BY p.date, p.id",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::{customers, invoices, payments, settings};

    async fn seed_invoice(db: &Db, customer: i64, price: i64, bp: i64, issue: bool) -> i64 {
        let inv = invoices::create_draft(
            db,
            customer,
            &[invoices::LineInput {
                item_id: None,
                description: "Work".into(),
                quantity: "1".into(),
                unit_price_minor: price,
                tax_rate_name: if bp > 0 {
                    "GST 10%".into()
                } else {
                    "No Tax".into()
                },
                tax_rate_bp: bp,
                tax_inclusive: false,
            }],
            None,
            "",
        )
        .await
        .unwrap();
        if issue {
            invoices::issue(db, inv).await.unwrap();
        }
        inv
    }

    #[sqlx::test]
    async fn aggregates_count_only_issued_documents(pool: Db) -> Result<(), DataError> {
        let mut business = settings::get(&pool).await?;
        business.business_name = "Test business".into();
        settings::update(&pool, &business).await?;
        let c = customers::create(
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

        // Two issued (one GST, one untaxed), one draft (excluded), one voided (excluded).
        seed_invoice(&pool, c, 10000, 1000, true).await; // $100 + $10 GST
        seed_invoice(&pool, c, 5000, 0, true).await; // $50 no tax
        seed_invoice(&pool, c, 99900, 1000, false).await; // draft — excluded
        let voided = seed_invoice(&pool, c, 77700, 1000, true).await;
        invoices::void(&pool, voided).await?;

        let tax = tax_summary(&pool, None, None).await?;
        assert_eq!(tax.len(), 2);
        let gst = tax.iter().find(|r| r.tax_rate_bp == 1000).unwrap();
        assert_eq!(gst.net_minor, 10000);
        assert_eq!(gst.tax_minor, 1000);
        let untaxed = tax.iter().find(|r| r.tax_rate_bp == 0).unwrap();
        assert_eq!(untaxed.net_minor, 5000);
        assert_eq!(untaxed.tax_minor, 0);

        let months = sales_by_month(&pool, None, None).await?;
        assert_eq!(months.len(), 1); // both issued today
        assert_eq!(months[0].invoice_count, 2);
        assert_eq!(months[0].gross_minor, 16000); // 110 + 50

        payments::record_payment(
            &pool,
            seed_invoice(&pool, c, 2000, 0, true).await,
            2000,
            "cash",
            "",
        )
        .await?;
        let by_customer = sales_by_customer(&pool, None, None).await?;
        assert_eq!(by_customer.len(), 1);
        assert_eq!(by_customer[0].invoice_count, 3);
        assert_eq!(by_customer[0].gross_minor, 18000);
        assert_eq!(by_customer[0].paid_minor, 2000);

        // Date bounds exclude everything when the window is in the past.
        assert!(tax_summary(&pool, Some("2000-01-01"), Some("2000-12-31"))
            .await?
            .is_empty());

        // Exports: full dump includes the draft + void; payment rows carry invoice + customer.
        assert_eq!(invoice_export_rows(&pool, None, None).await?.len(), 5);
        let pays = payment_export_rows(&pool, None, None).await?;
        assert_eq!(pays.len(), 1);
        assert_eq!(pays[0].customer, "Acme");
        assert_eq!(pays[0].amount_minor, 2000);
        assert!(sales_by_month(&pool, Some("2026-02-29"), None)
            .await
            .is_err());
        assert!(tax_summary(&pool, Some("2026-12-01"), Some("2026-01-01"))
            .await
            .is_err());
        Ok(())
    }
}
