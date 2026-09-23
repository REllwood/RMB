//! Reporting + CSV export commands. Aggregation happens in SQL (rmb-data); this layer formats
//! CSV (RFC-4180 quoting, amounts as plain major-unit decimals) and writes the chosen file.

use rmb_data::db::Db;
use rmb_data::repos::customers;
use rmb_data::repos::reports::{self, CustomerSalesRow, MonthlySalesRow, TaxSummaryRow};
use tauri::State;

use crate::error::AppError;

#[tauri::command]
pub async fn report_tax_summary(
    db: State<'_, Db>,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<TaxSummaryRow>, AppError> {
    Ok(reports::tax_summary(&db, from.as_deref(), to.as_deref()).await?)
}

#[tauri::command]
pub async fn report_sales_monthly(
    db: State<'_, Db>,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<MonthlySalesRow>, AppError> {
    Ok(reports::sales_by_month(&db, from.as_deref(), to.as_deref()).await?)
}

#[tauri::command]
pub async fn report_sales_customers(
    db: State<'_, Db>,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<CustomerSalesRow>, AppError> {
    Ok(reports::sales_by_customer(&db, from.as_deref(), to.as_deref()).await?)
}

/// RFC-4180 field quoting: wrap when the field contains a comma, quote, or newline; double
/// embedded quotes.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Keep user-entered text from becoming a spreadsheet formula when the CSV is opened. The leading
/// apostrophe is Excel/LibreOffice's explicit text marker. Numeric amount cells bypass this helper.
fn csv_text(s: impl Into<String>) -> String {
    let value = s.into();
    let first = value.trim_start().chars().next();
    if matches!(first, Some('=' | '+' | '-' | '@')) {
        format!("'{value}")
    } else {
        value
    }
}

fn csv_line(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",")
}

/// Plain major-unit decimal for spreadsheets (no symbol, no grouping): 12345 → "123.45".
fn csv_amount(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

fn write_csv(dest: &str, header: &[&str], rows: Vec<Vec<String>>) -> Result<(), AppError> {
    // UTF-8 BOM makes non-ASCII customer names open correctly in Windows Excel without an import
    // wizard. Rows otherwise use RFC-4180 quoting and CRLF line endings.
    let mut out = String::from('\u{FEFF}');
    out.push_str(&csv_line(
        &header.iter().map(|h| h.to_string()).collect::<Vec<_>>(),
    ));
    out.push_str("\r\n");
    for row in rows {
        out.push_str(&csv_line(&row));
        out.push_str("\r\n");
    }
    std::fs::write(dest, out).map_err(|e| AppError::Message(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn export_invoices_csv(
    db: State<'_, Db>,
    dest: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<(), AppError> {
    let rows = reports::invoice_export_rows(&db, from.as_deref(), to.as_deref()).await?;
    let data = rows
        .into_iter()
        .map(|r| {
            vec![
                csv_text(r.number.unwrap_or_else(|| format!("#{}", r.id))),
                csv_text(r.customer),
                r.status,
                r.issue_date.unwrap_or_default(),
                r.due_date.unwrap_or_default(),
                r.void_date.unwrap_or_default(),
                csv_amount(r.subtotal_minor),
                csv_amount(r.tax_minor),
                csv_amount(r.total_minor),
                csv_amount(r.paid_minor),
                csv_amount(r.balance_minor),
            ]
        })
        .collect();
    write_csv(
        &dest,
        &[
            "number",
            "customer",
            "status",
            "issue_date",
            "due_date",
            "void_date",
            "subtotal",
            "tax",
            "total",
            "paid",
            "balance",
        ],
        data,
    )
}

#[tauri::command]
pub async fn export_payments_csv(
    db: State<'_, Db>,
    dest: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<(), AppError> {
    let rows = reports::payment_export_rows(&db, from.as_deref(), to.as_deref()).await?;
    let data = rows
        .into_iter()
        .map(|r| {
            vec![
                r.date,
                csv_amount(r.amount_minor),
                csv_text(r.method),
                csv_text(r.reference),
                csv_text(r.invoice_number.unwrap_or_default()),
                csv_text(r.customer),
            ]
        })
        .collect();
    write_csv(
        &dest,
        &[
            "date",
            "amount",
            "method",
            "reference",
            "invoice",
            "customer",
        ],
        data,
    )
}

#[tauri::command]
pub async fn export_customers_csv(db: State<'_, Db>, dest: String) -> Result<(), AppError> {
    let rows = customers::export_rows(&db).await?;
    let data = rows
        .into_iter()
        .map(|c| {
            vec![
                csv_text(c.name),
                csv_text(c.email),
                csv_text(c.phone),
                csv_text(c.billing_address),
                csv_text(c.notes),
                c.created_at,
                c.deleted_at.unwrap_or_default(),
            ]
        })
        .collect();
    write_csv(
        &dest,
        &[
            "name",
            "email",
            "phone",
            "billing_address",
            "notes",
            "created_at",
            "deleted_at",
        ],
        data,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escaping_and_amounts() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("has,comma"), "\"has,comma\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
        assert_eq!(csv_text("=1+1"), "'=1+1");
        assert_eq!(csv_text("  @SUM(A1:A2)"), "'  @SUM(A1:A2)");
        assert_eq!(csv_text("ordinary text"), "ordinary text");
        assert_eq!(csv_line(&["a,b".into(), "c".into()]), "\"a,b\",c");
        assert_eq!(csv_amount(12345), "123.45");
        assert_eq!(csv_amount(5), "0.05");
        assert_eq!(csv_amount(-12345), "-123.45");
        assert_eq!(csv_amount(0), "0.00");
    }
}
