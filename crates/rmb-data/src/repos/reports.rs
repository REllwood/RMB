//! Reporting aggregates. Revenue reports are **accrual basis**: an invoice counts in the period
//! it was issued, and if it is later voided the reversal is reported in the period of the void
//! (like a credit note), so a period that has already been reported never changes afterwards.
//! Drafts never count. Cash figures (payments received, the payments export) go by payment date.
//! Optional date bounds are inclusive `YYYY-MM-DD`; `None` means unbounded (COALESCE sentinels
//! keep the SQL static for sqlx's safety check).

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
    /// Invoices issued in the month (including any voided later).
    pub invoice_count: i64,
    /// Invoices voided in the month; their amounts are subtracted from the month's totals.
    pub voided_count: i64,
    pub net_minor: i64,
    pub tax_minor: i64,
    pub gross_minor: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CustomerSalesRow {
    pub customer_id: i64,
    pub name: String,
    /// Invoices issued in the period.
    pub invoice_count: i64,
    /// Sales in the period, net of voids made in the period.
    pub gross_minor: i64,
    /// Cash received from the customer in the period (by payment date).
    pub paid_minor: i64,
}

/// Invoice amounts by period: `+` in the issue period, `-` in the void period. Callers filter the
/// `period_date` column and aggregate.
macro_rules! signed_invoices {
    () => {
        "SELECT i.id, i.customer_id, i.issue_date AS period_date, 1 AS sign, 0 AS voided, \
                i.subtotal_minor, i.tax_minor, i.total_minor \
         FROM invoice i \
         WHERE i.status IN ('issued', 'part_paid', 'paid', 'void') AND i.issue_date IS NOT NULL \
         UNION ALL \
         SELECT i.id, i.customer_id, i.void_date, -1, 1, \
                i.subtotal_minor, i.tax_minor, i.total_minor \
         FROM invoice i WHERE i.status = 'void' AND i.void_date IS NOT NULL"
    };
}

/// Tax per rate (the BAS / VAT-return view), from line-level snapshots, net of voids.
pub async fn tax_summary(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<TaxSummaryRow>, DataError> {
    validate_range(from, to)?;
    Ok(sqlx::query_as::<_, TaxSummaryRow>(
        "WITH signed AS ( \
           SELECT l.tax_rate_name, l.tax_rate_bp, i.issue_date AS period_date, \
                  l.net_minor AS net, l.tax_minor AS tax, l.gross_minor AS gross \
           FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id \
           WHERE i.status IN ('issued', 'part_paid', 'paid', 'void') AND i.issue_date IS NOT NULL \
           UNION ALL \
           SELECT l.tax_rate_name, l.tax_rate_bp, i.void_date, \
                  -l.net_minor, -l.tax_minor, -l.gross_minor \
           FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id \
           WHERE i.status = 'void' AND i.void_date IS NOT NULL \
         ) \
         SELECT tax_rate_name, tax_rate_bp, \
                COALESCE(SUM(net), 0) AS net_minor, COALESCE(SUM(tax), 0) AS tax_minor, \
                COALESCE(SUM(gross), 0) AS gross_minor \
         FROM signed \
         WHERE period_date >= COALESCE(?, '0000-00-00') AND period_date <= COALESCE(?, '9999-12-31') \
         GROUP BY tax_rate_name, tax_rate_bp \
         HAVING SUM(net) <> 0 OR SUM(tax) <> 0 \
         ORDER BY tax_rate_bp DESC, tax_rate_name",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?)
}

/// Sales per calendar month: issued in the month, less voids made in the month.
pub async fn sales_by_month(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<MonthlySalesRow>, DataError> {
    validate_range(from, to)?;
    Ok(sqlx::query_as::<_, MonthlySalesRow>(concat!(
        "WITH signed AS (",
        signed_invoices!(),
        ") SELECT substr(period_date, 1, 7) AS month, \
                  COALESCE(SUM(CASE WHEN sign = 1 THEN 1 ELSE 0 END), 0) AS invoice_count, \
                  COALESCE(SUM(voided), 0) AS voided_count, \
                  COALESCE(SUM(sign * subtotal_minor), 0) AS net_minor, \
                  COALESCE(SUM(sign * tax_minor), 0) AS tax_minor, \
                  COALESCE(SUM(sign * total_minor), 0) AS gross_minor \
           FROM signed \
           WHERE period_date >= COALESCE(?, '0000-00-00') \
             AND period_date <= COALESCE(?, '9999-12-31') \
           GROUP BY month ORDER BY month DESC"
    ))
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?)
}

/// Sales (net of voids) and cash received per customer in the period, biggest first.
pub async fn sales_by_customer(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<CustomerSalesRow>, DataError> {
    validate_range(from, to)?;
    Ok(sqlx::query_as::<_, CustomerSalesRow>(concat!(
        "WITH signed AS (",
        signed_invoices!(),
        "), sales AS ( \
           SELECT customer_id, SUM(CASE WHEN sign = 1 THEN 1 ELSE 0 END) AS invoice_count, \
                  SUM(sign * total_minor) AS gross \
           FROM signed \
           WHERE period_date >= COALESCE(?1, '0000-00-00') \
             AND period_date <= COALESCE(?2, '9999-12-31') \
           GROUP BY customer_id \
         ), received AS ( \
           SELECT i.customer_id, SUM(a.amount_minor) AS paid \
           FROM payment_allocation a JOIN payment p ON p.id = a.payment_id \
           JOIN invoice i ON i.id = a.invoice_id \
           WHERE substr(p.date, 1, 10) >= COALESCE(?1, '0000-00-00') \
             AND substr(p.date, 1, 10) <= COALESCE(?2, '9999-12-31') \
           GROUP BY i.customer_id \
         ) \
         SELECT ids.customer_id, COALESCE(c.name, '(removed)') AS name, \
                COALESCE(s.invoice_count, 0) AS invoice_count, COALESCE(s.gross, 0) AS gross_minor, \
                COALESCE(r.paid, 0) AS paid_minor \
         FROM (SELECT customer_id FROM sales UNION SELECT customer_id FROM received) ids \
         LEFT JOIN sales s ON s.customer_id = ids.customer_id \
         LEFT JOIN received r ON r.customer_id = ids.customer_id \
         LEFT JOIN customer c ON c.id = ids.customer_id \
         ORDER BY gross_minor DESC, paid_minor DESC, name"
    ))
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
    pub void_date: Option<String>,
    pub subtotal_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub paid_minor: i64,
    /// What is still owed: zero for void and fully paid invoices.
    pub balance_minor: i64,
}

/// Flat rows of issued invoices (drafts are not financial documents and are never exported),
/// optionally limited to those issued in the range. Voided invoices are included with their void
/// date and a zero balance. The customer is the name frozen on the invoice.
pub async fn invoice_export_rows(
    db: &Db,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<InvoiceExportRow>, DataError> {
    validate_range(from, to)?;
    Ok(sqlx::query_as::<_, InvoiceExportRow>(
        "SELECT i.id, i.number, \
                COALESCE(CASE WHEN json_valid(i.customer_snapshot) \
                              THEN json_extract(i.customer_snapshot, '$.name') END, \
                         c.name, '(removed)') AS customer, \
                i.status, i.issue_date, i.due_date, i.void_date, i.subtotal_minor, i.tax_minor, \
                i.total_minor, COALESCE(a.paid, 0) AS paid_minor, \
                CASE WHEN i.status = 'void' THEN 0 \
                     ELSE MAX(i.total_minor - COALESCE(a.paid, 0), 0) END AS balance_minor \
         FROM invoice i LEFT JOIN customer c ON c.id = i.customer_id \
         LEFT JOIN (SELECT invoice_id, SUM(amount_minor) AS paid FROM payment_allocation \
                    GROUP BY invoice_id) a ON a.invoice_id = i.id \
         WHERE i.status <> 'draft' AND i.issue_date IS NOT NULL \
           AND i.issue_date >= COALESCE(?, '0000-00-00') AND i.issue_date <= COALESCE(?, '9999-12-31') \
         ORDER BY i.issue_date, i.id",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?)
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
         COALESCE(CASE WHEN json_valid(i.customer_snapshot) \
                       THEN json_extract(i.customer_snapshot, '$.name') END, \
                  c.name, '(removed)') AS customer \
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

        // Two issued (one GST, one untaxed), one draft (excluded), one issued and voided today
        // (its sale and its reversal cancel out within the same period).
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
        assert_eq!(months.len(), 1); // everything happened today
        assert_eq!(months[0].invoice_count, 3);
        assert_eq!(months[0].voided_count, 1);
        assert_eq!(months[0].gross_minor, 16000); // 110 + 50 + 777 − 777

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
        assert_eq!(by_customer[0].invoice_count, 4);
        assert_eq!(by_customer[0].gross_minor, 18000);
        assert_eq!(by_customer[0].paid_minor, 2000);

        // Date bounds exclude everything when the window is in the past.
        assert!(tax_summary(&pool, Some("2000-01-01"), Some("2000-12-31"))
            .await?
            .is_empty());

        // Exports: drafts are never exported; the void is listed with a zero balance.
        let rows = invoice_export_rows(&pool, None, None).await?;
        assert_eq!(rows.len(), 4);
        let void_row = rows.iter().find(|r| r.status == "void").unwrap();
        assert_eq!(void_row.balance_minor, 0);
        assert!(void_row.void_date.is_some());
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

    #[sqlx::test]
    async fn a_later_void_is_reported_in_its_own_period(pool: Db) -> Result<(), DataError> {
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
        let inv = seed_invoice(&pool, c, 10000, 1000, true).await;
        // Issued in March (a period already reported), voided now.
        sqlx::query("UPDATE invoice SET issue_date = '2026-03-15' WHERE id = ?")
            .bind(inv)
            .execute(&pool)
            .await?;
        let march = |db: Db| async move {
            tax_summary(&db, Some("2026-03-01"), Some("2026-03-31"))
                .await
                .unwrap()
        };
        let before = march(pool.clone()).await;
        invoices::void(&pool, inv).await?;
        assert_eq!(
            march(pool.clone()).await[0].tax_minor,
            before[0].tax_minor,
            "a reported period never changes afterwards"
        );

        let today = crate::db::today_local(&pool).await?;
        let now = tax_summary(&pool, Some(&today), Some(&today)).await?;
        assert_eq!(now[0].net_minor, -10000);
        assert_eq!(now[0].tax_minor, -1000);
        let month = sales_by_month(&pool, Some(&today), Some(&today)).await?;
        assert_eq!(month[0].voided_count, 1);
        assert_eq!(month[0].gross_minor, -11000);
        Ok(())
    }

    #[sqlx::test]
    async fn customer_cash_is_counted_by_payment_date(pool: Db) -> Result<(), DataError> {
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
        let inv = seed_invoice(&pool, c, 10000, 0, true).await;
        sqlx::query("UPDATE invoice SET issue_date = '2026-01-10' WHERE id = ?")
            .bind(inv)
            .execute(&pool)
            .await?;
        payments::record_payment_dated(&pool, inv, 4000, "cash", "", Some("2026-02-03")).await?;

        let january = sales_by_customer(&pool, Some("2026-01-01"), Some("2026-01-31")).await?;
        assert_eq!(january[0].gross_minor, 10000);
        assert_eq!(january[0].paid_minor, 0, "paid in February, not January");
        let february = sales_by_customer(&pool, Some("2026-02-01"), Some("2026-02-28")).await?;
        assert_eq!(february[0].gross_minor, 0);
        assert_eq!(february[0].paid_minor, 4000);
        Ok(())
    }
}
