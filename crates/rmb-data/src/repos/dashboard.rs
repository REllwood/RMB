//! Dashboard aggregates (money owed, counts, low stock).

use serde::Serialize;
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LowStockItem {
    pub id: i64,
    pub name: String,
    pub qty_on_hand: i64,
    pub reorder_point: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardSummary {
    pub outstanding_minor: i64,
    pub draft_count: i64,
    pub unpaid_count: i64,
    pub overdue_count: i64,
    pub paid_count: i64,
    pub low_stock: Vec<LowStockItem>,
}

pub async fn summary(db: &Db) -> Result<DashboardSummary, DataError> {
    let outstanding_minor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(i.total_minor - COALESCE(a.paid, 0)), 0) FROM invoice i \
         LEFT JOIN (SELECT invoice_id, SUM(amount_minor) AS paid FROM payment_allocation GROUP BY invoice_id) a \
         ON a.invoice_id = i.id WHERE i.status IN ('issued', 'part_paid')",
    )
    .fetch_one(db)
    .await?;
    let draft_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM invoice WHERE status = 'draft'")
            .fetch_one(db)
            .await?;
    let unpaid_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM invoice WHERE status IN ('issued', 'part_paid')")
            .fetch_one(db)
            .await?;
    // Overdue = unpaid past its due date, judged in the user's local timezone.
    let overdue_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoice WHERE status IN ('issued', 'part_paid') \
         AND due_date IS NOT NULL AND due_date < date('now','localtime')",
    )
    .fetch_one(db)
    .await?;
    let paid_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice WHERE status = 'paid'")
        .fetch_one(db)
        .await?;
    let low_stock = sqlx::query_as::<_, LowStockItem>(
        "SELECT id, name, qty_on_hand, reorder_point FROM item \
         WHERE deleted_at IS NULL AND tracked = 1 AND reorder_point IS NOT NULL \
         AND qty_on_hand <= reorder_point ORDER BY name",
    )
    .fetch_all(db)
    .await?;

    Ok(DashboardSummary {
        outstanding_minor,
        draft_count,
        unpaid_count,
        overdue_count,
        paid_count,
        low_stock,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::{customers, invoices, payments};

    #[sqlx::test]
    async fn summary_counts_outstanding(pool: Db) -> Result<(), DataError> {
        let c = customers::create(
            &pool,
            &customers::CustomerInput {
                name: "C".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await?;
        let inv = invoices::create_draft(
            &pool,
            c,
            &[invoices::LineInput {
                item_id: None,
                description: "x".into(),
                quantity: "1".into(),
                unit_price_minor: 10000,
                tax_rate_name: "No Tax".into(),
                tax_rate_bp: 0,
                tax_inclusive: false,
            }],
            None,
            "",
        )
        .await?;
        invoices::issue(&pool, inv).await?;
        payments::record_payment(&pool, inv, 4000, "cash", "").await?;

        let s = summary(&pool).await?;
        assert_eq!(s.outstanding_minor, 6000); // 100.00 - 40.00
        assert_eq!(s.unpaid_count, 1);
        assert_eq!(s.overdue_count, 0); // no due date → never overdue

        // An unpaid invoice past its due date counts as overdue.
        let late = invoices::create_draft(
            &pool,
            c,
            &[invoices::LineInput {
                item_id: None,
                description: "y".into(),
                quantity: "1".into(),
                unit_price_minor: 5000,
                tax_rate_name: "No Tax".into(),
                tax_rate_bp: 0,
                tax_inclusive: false,
            }],
            Some("2000-01-01"),
            "",
        )
        .await?;
        invoices::issue(&pool, late).await?;
        assert_eq!(summary(&pool).await?.overdue_count, 1);
        Ok(())
    }
}
