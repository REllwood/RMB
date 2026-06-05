//! Quote (estimate) repository. Mirrors invoices (no stock/payments). A quote can convert into a
//! draft invoice, copying its lines and linking both directions.

use rmb_domain::document::total_lines;
use rmb_domain::numbering::format_number;
use rmb_domain::status::QuoteStatus;
use rmb_domain::tax::line_tax;
use serde::Serialize;
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;
use crate::repos::invoices::{self, to_doc_line, LineInput};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct QuoteRow {
    pub id: i64,
    pub customer_id: i64,
    pub number: Option<String>,
    pub status: String,
    pub valid_until: Option<String>,
    pub subtotal_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub notes: String,
    pub converted_invoice_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct QuoteLineRow {
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
pub struct QuoteDetail {
    pub quote: QuoteRow,
    pub lines: Vec<QuoteLineRow>,
}

pub async fn create_draft(
    db: &Db,
    customer_id: i64,
    lines: &[LineInput],
    valid_until: Option<&str>,
    notes: &str,
) -> Result<i64, DataError> {
    let doc_lines = lines
        .iter()
        .map(to_doc_line)
        .collect::<Result<Vec<_>, _>>()?;
    let totals = total_lines(&doc_lines);
    let tax_summary = serde_json::to_string(&totals.tax_summary).unwrap_or_else(|_| "[]".into());

    let mut tx = db.begin().await?;
    // Quotes are free-form; assign a number at create from the quote counter.
    let (prefix, seq, pad) = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT quote_prefix, quote_next_seq, number_pad FROM settings WHERE id = 1",
    )
    .fetch_one(&mut *tx)
    .await?;
    let number = format_number(&prefix, seq, pad.max(0) as usize);
    sqlx::query("UPDATE settings SET quote_next_seq = quote_next_seq + 1 WHERE id = 1")
        .execute(&mut *tx)
        .await?;

    let quote_id = sqlx::query(
        "INSERT INTO quote (customer_id, number, status, valid_until, subtotal_minor, tax_minor, total_minor, tax_summary, notes) \
         VALUES (?, ?, 'draft', ?, ?, ?, ?, ?, ?)",
    )
    .bind(customer_id)
    .bind(&number)
    .bind(valid_until)
    .bind(totals.subtotal.minor())
    .bind(totals.tax_total.minor())
    .bind(totals.total.minor())
    .bind(tax_summary)
    .bind(notes)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    for (i, (l, dl)) in lines.iter().zip(doc_lines.iter()).enumerate() {
        let lt = line_tax(dl.line_amount(), &dl.tax_rate);
        sqlx::query(
            "INSERT INTO quote_line (quote_id, item_id, description, quantity, unit_price_minor, \
             tax_rate_name, tax_rate_bp, tax_inclusive, net_minor, tax_minor, gross_minor, line_order) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(quote_id)
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
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(quote_id)
}

pub async fn list(db: &Db) -> Result<Vec<QuoteRow>, DataError> {
    Ok(sqlx::query_as::<_, QuoteRow>(
        "SELECT id, customer_id, number, status, valid_until, subtotal_minor, tax_minor, \
         total_minor, notes, converted_invoice_id, created_at FROM quote \
         WHERE deleted_at IS NULL ORDER BY id DESC",
    )
    .fetch_all(db)
    .await?)
}

pub async fn get_detail(db: &Db, id: i64) -> Result<Option<QuoteDetail>, DataError> {
    let quote = sqlx::query_as::<_, QuoteRow>(
        "SELECT id, customer_id, number, status, valid_until, subtotal_minor, tax_minor, \
         total_minor, notes, converted_invoice_id, created_at FROM quote \
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    let Some(quote) = quote else { return Ok(None) };
    let lines = sqlx::query_as::<_, QuoteLineRow>(
        "SELECT id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, \
         tax_inclusive, net_minor, tax_minor, gross_minor FROM quote_line WHERE quote_id = ? ORDER BY line_order",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    Ok(Some(QuoteDetail { quote, lines }))
}

pub async fn set_status(db: &Db, id: i64, to: &str) -> Result<(), DataError> {
    let current: Option<String> =
        sqlx::query_scalar("SELECT status FROM quote WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(db)
            .await?;
    let current = current.ok_or_else(|| DataError::Other("quote not found".into()))?;
    let from = QuoteStatus::from_db(&current)
        .ok_or_else(|| DataError::Other("invalid current status".into()))?;
    let target = QuoteStatus::from_db(to)
        .ok_or_else(|| DataError::Other(format!("unknown status '{to}'")))?;
    if !from.can_transition_to(target) {
        return Err(DataError::Other(format!(
            "cannot move a quote from {current} to {to}"
        )));
    }
    sqlx::query("UPDATE quote SET status = ? WHERE id = ?")
        .bind(target.as_db())
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete(db: &Db, id: i64) -> Result<(), DataError> {
    sqlx::query("UPDATE quote SET deleted_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Convert a quote into a draft invoice (copying its lines), link both, and mark it converted.
pub async fn convert_to_invoice(db: &Db, id: i64) -> Result<i64, DataError> {
    let detail = get_detail(db, id)
        .await?
        .ok_or_else(|| DataError::Other("quote not found".into()))?;
    let from = QuoteStatus::from_db(&detail.quote.status)
        .ok_or_else(|| DataError::Other("invalid status".into()))?;
    if matches!(
        from,
        QuoteStatus::Converted | QuoteStatus::Declined | QuoteStatus::Expired
    ) {
        return Err(DataError::Other(format!(
            "a {} quote cannot be converted",
            detail.quote.status
        )));
    }

    let lines: Vec<LineInput> = detail
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

    let invoice_id = invoices::create_draft(
        db,
        detail.quote.customer_id,
        &lines,
        None,
        &detail.quote.notes,
    )
    .await?;
    sqlx::query("UPDATE quote SET status = 'converted', converted_invoice_id = ? WHERE id = ?")
        .bind(invoice_id)
        .bind(id)
        .execute(db)
        .await?;
    sqlx::query("UPDATE invoice SET source_quote_id = ? WHERE id = ?")
        .bind(id)
        .bind(invoice_id)
        .execute(db)
        .await?;
    Ok(invoice_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::customers;

    #[sqlx::test]
    async fn create_status_transitions_and_convert(pool: Db) -> Result<(), DataError> {
        let customer = customers::create(
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
        let line = LineInput {
            item_id: None,
            description: "Design work".into(),
            quantity: "10".into(),
            unit_price_minor: 5000,
            tax_rate_name: "VAT 20%".into(),
            tax_rate_bp: 2000,
            tax_inclusive: false,
        };
        let q = create_draft(&pool, customer, &[line], Some("2026-07-01"), "thanks").await?;

        let d = get_detail(&pool, q).await?.unwrap();
        assert_eq!(d.quote.status, "draft");
        assert!(d.quote.number.is_some());
        assert_eq!(d.quote.subtotal_minor, 50000);
        assert_eq!(d.quote.tax_minor, 10000);
        assert_eq!(d.quote.total_minor, 60000);

        set_status(&pool, q, "sent").await?;
        set_status(&pool, q, "accepted").await?;
        assert!(set_status(&pool, q, "draft").await.is_err()); // illegal accepted→draft

        let invoice_id = convert_to_invoice(&pool, q).await?;
        assert_eq!(
            get_detail(&pool, q).await?.unwrap().quote.status,
            "converted"
        );
        let inv_detail = invoices::get_detail(&pool, invoice_id).await?.unwrap();
        assert_eq!(inv_detail.invoice.total_minor, 60000);
        Ok(())
    }
}
