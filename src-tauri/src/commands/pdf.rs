//! PDF export commands for invoices + quotes.

use rmb_data::db::Db;
use rmb_data::repos::settings::Settings;
use rmb_data::repos::{customers, invoices, quotes, settings};
use serde_json::json;
use tauri::State;

use crate::error::AppError;
use crate::pdf;

fn business_lines(s: &Settings) -> Vec<String> {
    let mut v = Vec::new();
    if !s.address.is_empty() {
        v.push(s.address.clone());
    }
    let contact: Vec<String> = [s.email.clone(), s.phone.clone()]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect();
    if !contact.is_empty() {
        v.push(contact.join(" · "));
    }
    if !s.tax_number.is_empty() {
        v.push(format!("{}: {}", s.tax_label, s.tax_number));
    }
    v
}

fn customer_lines(c: Option<&customers::Customer>) -> Vec<String> {
    match c {
        Some(c) => {
            let mut v = vec![c.name.clone()];
            if !c.billing_address.is_empty() {
                v.push(c.billing_address.clone());
            }
            if !c.email.is_empty() {
                v.push(c.email.clone());
            }
            v
        }
        None => vec!["(customer removed)".to_string()],
    }
}

fn write_pdf(json_data: &str, dest: &str) -> Result<(), AppError> {
    let bytes = pdf::render(json_data).map_err(AppError::Message)?;
    std::fs::write(dest, bytes).map_err(|e| AppError::Message(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn export_invoice_pdf(db: State<'_, Db>, id: i64, dest: String) -> Result<(), AppError> {
    let detail = invoices::get_detail(&db, id)
        .await?
        .ok_or_else(|| AppError::Message("invoice not found".into()))?;
    let s = settings::get(&db).await?;
    let customer = customers::get(&db, detail.invoice.customer_id).await?;
    let cur = &s.currency;

    let number = detail
        .invoice
        .number
        .clone()
        .unwrap_or_else(|| format!("Draft #{}", detail.invoice.id));

    let lines: Vec<[String; 4]> = detail
        .lines
        .iter()
        .map(|l| {
            [
                l.description.clone(),
                l.quantity.clone(),
                pdf::fmt_money(l.unit_price_minor, cur),
                pdf::fmt_money(l.gross_minor, cur),
            ]
        })
        .collect();

    let mut totals: Vec<[String; 2]> = vec![
        [
            "Subtotal".into(),
            pdf::fmt_money(detail.invoice.subtotal_minor, cur),
        ],
        ["Tax".into(), pdf::fmt_money(detail.invoice.tax_minor, cur)],
        [
            "Total".into(),
            pdf::fmt_money(detail.invoice.total_minor, cur),
        ],
    ];
    if detail.amount_paid_minor > 0 {
        totals.push(["Paid".into(), pdf::fmt_money(detail.amount_paid_minor, cur)]);
    }
    totals.push([
        "Balance due".into(),
        pdf::fmt_money(detail.invoice.total_minor - detail.amount_paid_minor, cur),
    ]);

    let mut meta = vec![number.clone()];
    if let Some(date) = &detail.invoice.issue_date {
        meta.push(format!("Date: {date}"));
    }
    if let Some(due) = &detail.invoice.due_date {
        meta.push(format!("Due: {due}"));
    }

    let data = json!({
        "title": format!("Invoice {number}"),
        "kind": "INVOICE",
        "business_name": s.business_name,
        "business_lines": business_lines(&s),
        "meta": meta,
        "customer_block": customer_lines(customer.as_ref()),
        "lines": lines,
        "totals": totals,
        "notes": detail.invoice.notes,
    })
    .to_string();

    write_pdf(&data, &dest)
}

#[tauri::command]
pub async fn export_quote_pdf(db: State<'_, Db>, id: i64, dest: String) -> Result<(), AppError> {
    let detail = quotes::get_detail(&db, id)
        .await?
        .ok_or_else(|| AppError::Message("quote not found".into()))?;
    let s = settings::get(&db).await?;
    let customer = customers::get(&db, detail.quote.customer_id).await?;
    let cur = &s.currency;

    let number = detail
        .quote
        .number
        .clone()
        .unwrap_or_else(|| format!("Quote #{}", detail.quote.id));

    let lines: Vec<[String; 4]> = detail
        .lines
        .iter()
        .map(|l| {
            [
                l.description.clone(),
                l.quantity.clone(),
                pdf::fmt_money(l.unit_price_minor, cur),
                pdf::fmt_money(l.gross_minor, cur),
            ]
        })
        .collect();

    let totals: Vec<[String; 2]> = vec![
        [
            "Subtotal".into(),
            pdf::fmt_money(detail.quote.subtotal_minor, cur),
        ],
        ["Tax".into(), pdf::fmt_money(detail.quote.tax_minor, cur)],
        [
            "Total".into(),
            pdf::fmt_money(detail.quote.total_minor, cur),
        ],
    ];

    let mut meta = vec![number.clone()];
    if let Some(valid) = &detail.quote.valid_until {
        meta.push(format!("Valid until: {valid}"));
    }

    let data = json!({
        "title": format!("Quote {number}"),
        "kind": "QUOTE",
        "business_name": s.business_name,
        "business_lines": business_lines(&s),
        "meta": meta,
        "customer_block": customer_lines(customer.as_ref()),
        "lines": lines,
        "totals": totals,
        "notes": detail.quote.notes,
    })
    .to_string();

    write_pdf(&data, &dest)
}
