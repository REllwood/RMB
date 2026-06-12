//! PDF export commands for invoices, quotes, and payment receipts.

use rmb_data::db::Db;
use rmb_data::repos::settings::Settings;
use rmb_data::repos::{customers, invoices, payments, quotes, settings};
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

/// Load the configured logo as (bytes, typst format). Any problem (moved file, odd extension)
/// just means an unbranded PDF — exporting must never fail because of the logo.
fn load_logo(s: &Settings) -> Option<(Vec<u8>, &'static str)> {
    let path = s.logo_path.as_deref()?;
    let format = match std::path::Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "png",
        "jpg" | "jpeg" => "jpg",
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some((bytes, format))
}

fn write_pdf(json_data: &str, logo: Option<(Vec<u8>, &str)>, dest: &str) -> Result<(), AppError> {
    let bytes = pdf::render(json_data, logo).map_err(AppError::Message)?;
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

    write_pdf(&data, load_logo(&s), &dest)
}

/// Receipt for an invoice's recorded payments — proof of what was paid and what remains.
#[tauri::command]
pub async fn export_receipt_pdf(db: State<'_, Db>, id: i64, dest: String) -> Result<(), AppError> {
    let detail = invoices::get_detail(&db, id)
        .await?
        .ok_or_else(|| AppError::Message("invoice not found".into()))?;
    let pays = payments::list_for_invoice(&db, id).await?;
    if pays.is_empty() {
        return Err(AppError::Message(
            "no payments recorded — nothing to receipt".into(),
        ));
    }
    let s = settings::get(&db).await?;
    let customer = customers::get(&db, detail.invoice.customer_id).await?;
    let cur = &s.currency;

    let number = detail
        .invoice
        .number
        .clone()
        .unwrap_or_else(|| format!("Draft #{}", detail.invoice.id));
    let today = rmb_data::db::today_local(&db).await?;

    // Payments listed oldest-first on the receipt.
    let lines: Vec<[String; 4]> = pays
        .iter()
        .rev()
        .map(|p| {
            let method = if p.method.is_empty() {
                "payment".to_string()
            } else {
                p.method.clone()
            };
            [
                format!(
                    "Payment — {method}{}",
                    if p.reference.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", p.reference)
                    }
                ),
                String::new(),
                p.date.chars().take(10).collect(),
                pdf::fmt_money(p.amount_minor, cur),
            ]
        })
        .collect();

    let totals: Vec<[String; 2]> = vec![
        [
            format!("Invoice total ({number})"),
            pdf::fmt_money(detail.invoice.total_minor, cur),
        ],
        ["Paid".into(), pdf::fmt_money(detail.amount_paid_minor, cur)],
        [
            "Balance".into(),
            pdf::fmt_money(detail.invoice.total_minor - detail.amount_paid_minor, cur),
        ],
    ];

    let data = json!({
        "title": format!("Receipt — {number}"),
        "kind": "RECEIPT",
        "business_name": s.business_name,
        "business_lines": business_lines(&s),
        "meta": [format!("For {number}"), format!("Date: {today}")],
        "customer_block": customer_lines(customer.as_ref()),
        "columns": ["Payment", "", "Date", "Amount"],
        "lines": lines,
        "totals": totals,
        "notes": "Thank you for your payment.",
    })
    .to_string();

    write_pdf(&data, load_logo(&s), &dest)
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

    write_pdf(&data, load_logo(&s), &dest)
}
