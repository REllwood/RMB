//! PDF export commands for invoices, quotes, and payment receipts.

use rmb_data::db::Db;
use rmb_data::repos::settings::Settings;
use rmb_data::repos::{customers, invoices, payments, quotes, settings};
use serde::Deserialize;
use serde_json::json;
use tauri::State;

use crate::error::AppError;
use crate::pdf;

fn business_lines_from(
    address: &str,
    email: &str,
    phone: &str,
    tax_label: &str,
    tax_number: &str,
) -> Vec<String> {
    let mut v = Vec::new();
    if !address.is_empty() {
        v.push(address.to_owned());
    }
    let contact: Vec<String> = [email, phone]
        .into_iter()
        .filter(|x| !x.is_empty())
        .map(str::to_owned)
        .collect();
    if !contact.is_empty() {
        v.push(contact.join(" · "));
    }
    if !tax_number.is_empty() {
        v.push(format!("{tax_label}: {tax_number}"));
    }
    v
}

fn business_lines(s: &Settings) -> Vec<String> {
    business_lines_from(&s.address, &s.email, &s.phone, &s.tax_label, &s.tax_number)
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

#[derive(Debug, Deserialize)]
struct BusinessSnapshot {
    name: String,
    address: String,
    email: String,
    phone: String,
    logo_path: Option<String>,
    tax_label: String,
    tax_number: String,
    #[serde(default)]
    currency: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CustomerSnapshot {
    name: String,
    email: String,
    #[allow(dead_code)]
    phone: String,
    address: String,
}

#[derive(Debug, Deserialize)]
struct StoredTaxSummary {
    rate_name: String,
    rate_bp: i64,
    taxable_net: i64,
    tax: i64,
}

struct InvoiceIdentity {
    business_name: String,
    business_lines: Vec<String>,
    customer_lines: Vec<String>,
    currency: String,
    logo: Option<(Vec<u8>, String)>,
}

/// Issued documents use their frozen identity/currency snapshots. Drafts and older invoices that
/// pre-date snapshot fields fall back to current records, preserving backwards compatibility.
fn invoice_identity(
    business_snapshot: Option<&str>,
    customer_snapshot: Option<&str>,
    current: &Settings,
    customer: Option<&customers::Customer>,
    frozen_logo: Option<(Vec<u8>, String)>,
    current_logo: Option<(Vec<u8>, String)>,
) -> InvoiceIdentity {
    let business =
        business_snapshot.and_then(|raw| serde_json::from_str::<BusinessSnapshot>(raw).ok());
    let frozen_customer =
        customer_snapshot.and_then(|raw| serde_json::from_str::<CustomerSnapshot>(raw).ok());

    let (business_name, lines, currency, logo) = match business {
        Some(b) => {
            let currency = b
                .currency
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| current.currency.clone());
            let lines =
                business_lines_from(&b.address, &b.email, &b.phone, &b.tax_label, &b.tax_number);
            let logo = frozen_logo.or_else(|| load_logo_path(b.logo_path.as_deref()));
            (b.name, lines, currency, logo)
        }
        None => (
            current.business_name.clone(),
            business_lines(current),
            current.currency.clone(),
            current_logo.or_else(|| load_logo_path(current.logo_path.as_deref())),
        ),
    };

    let customer_lines = match frozen_customer {
        Some(c) => {
            let mut lines = vec![c.name];
            if !c.address.is_empty() {
                lines.push(c.address);
            }
            if !c.email.is_empty() {
                lines.push(c.email);
            }
            lines
        }
        None => customer_lines(customer),
    };

    InvoiceIdentity {
        business_name,
        business_lines: lines,
        customer_lines,
        currency,
        logo,
    }
}

fn document_totals(
    subtotal_minor: i64,
    tax_minor: i64,
    total_minor: i64,
    tax_summary: &str,
    currency: &str,
) -> Vec<[String; 2]> {
    let mut rows = vec![["Subtotal".into(), pdf::fmt_money(subtotal_minor, currency)]];
    let summaries = serde_json::from_str::<Vec<StoredTaxSummary>>(tax_summary).unwrap_or_default();
    if summaries.is_empty() {
        rows.push(["Tax".into(), pdf::fmt_money(tax_minor, currency)]);
    } else {
        for summary in &summaries {
            let percentage = format!("{:.2}%", summary.rate_bp as f64 / 100.0);
            rows.push([
                format!(
                    "{} ({percentage}) on {}",
                    summary.rate_name,
                    pdf::fmt_money(summary.taxable_net, currency)
                ),
                pdf::fmt_money(summary.tax, currency),
            ]);
        }
        if summaries.len() > 1 {
            rows.push(["Tax total".into(), pdf::fmt_money(tax_minor, currency)]);
        }
    }
    rows.push(["Total".into(), pdf::fmt_money(total_minor, currency)]);
    rows
}

/// Load the configured logo as (bytes, typst format). Any problem (moved file, odd extension)
/// just means an unbranded PDF — exporting must never fail because of the logo.
fn load_logo_path(path: Option<&str>) -> Option<(Vec<u8>, String)> {
    let path = path?;
    let format = match std::path::Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "png".to_owned(),
        "jpg" | "jpeg" => "jpg".to_owned(),
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some((bytes, format))
}

fn write_pdf(json_data: &str, logo: Option<(Vec<u8>, String)>, dest: &str) -> Result<(), AppError> {
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
    let current_logo = settings::get_logo_asset(&db)
        .await?
        .map(|asset| (asset.data, asset.format));
    let customer = customers::get(&db, detail.invoice.customer_id).await?;
    let frozen_logo = match (
        detail.business_logo_data.clone(),
        detail.business_logo_format.clone(),
    ) {
        (Some(data), Some(format)) if matches!(format.as_str(), "png" | "jpg") => {
            Some((data, format))
        }
        _ => None,
    };
    let identity = invoice_identity(
        detail.business_snapshot.as_deref(),
        detail.customer_snapshot.as_deref(),
        &s,
        customer.as_ref(),
        frozen_logo,
        current_logo,
    );
    let cur = &identity.currency;

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

    let mut totals = document_totals(
        detail.invoice.subtotal_minor,
        detail.invoice.tax_minor,
        detail.invoice.total_minor,
        &detail.tax_summary,
        cur,
    );
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
        "business_name": identity.business_name,
        "business_lines": identity.business_lines,
        "meta": meta,
        "customer_block": identity.customer_lines,
        "lines": lines,
        "totals": totals,
        "notes": detail.invoice.notes,
    })
    .to_string();

    write_pdf(&data, identity.logo, &dest)
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
    let current_logo = settings::get_logo_asset(&db)
        .await?
        .map(|asset| (asset.data, asset.format));
    let customer = customers::get(&db, detail.invoice.customer_id).await?;
    let frozen_logo = match (
        detail.business_logo_data.clone(),
        detail.business_logo_format.clone(),
    ) {
        (Some(data), Some(format)) if matches!(format.as_str(), "png" | "jpg") => {
            Some((data, format))
        }
        _ => None,
    };
    let identity = invoice_identity(
        detail.business_snapshot.as_deref(),
        detail.customer_snapshot.as_deref(),
        &s,
        customer.as_ref(),
        frozen_logo,
        current_logo,
    );
    let cur = &identity.currency;

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
        "business_name": identity.business_name,
        "business_lines": identity.business_lines,
        "meta": [format!("For {number}"), format!("Date: {today}")],
        "customer_block": identity.customer_lines,
        "columns": ["Payment", "", "Date", "Amount"],
        "lines": lines,
        "totals": totals,
        "notes": "Thank you for your payment.",
    })
    .to_string();

    write_pdf(&data, identity.logo, &dest)
}

#[tauri::command]
pub async fn export_quote_pdf(db: State<'_, Db>, id: i64, dest: String) -> Result<(), AppError> {
    let detail = quotes::get_detail(&db, id)
        .await?
        .ok_or_else(|| AppError::Message("quote not found".into()))?;
    let s = settings::get(&db).await?;
    let logo = settings::get_logo_asset(&db)
        .await?
        .map(|asset| (asset.data, asset.format))
        .or_else(|| load_logo_path(s.logo_path.as_deref()));
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

    let totals = document_totals(
        detail.quote.subtotal_minor,
        detail.quote.tax_minor,
        detail.quote.total_minor,
        &detail.tax_summary,
        cur,
    );

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

    write_pdf(&data, logo, &dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_settings() -> Settings {
        Settings {
            business_name: "New business".into(),
            address: "New address".into(),
            email: "new@example.com".into(),
            phone: "222".into(),
            logo_path: None,
            currency: "USD".into(),
            tax_label: "Tax".into(),
            tax_number: "NEW".into(),
            prices_tax_inclusive: false,
            invoice_prefix: "INV-".into(),
            invoice_next_seq: 2,
            quote_prefix: "Q-".into(),
            quote_next_seq: 2,
            number_pad: 4,
            default_tax_rate_id: None,
            currency_locked: false,
        }
    }

    #[test]
    fn issued_identity_uses_frozen_snapshot() {
        let business = json!({
            "name": "Original business",
            "address": "Original address",
            "email": "old@example.com",
            "phone": "111",
            "logo_path": null,
            "tax_label": "GST",
            "tax_number": "OLD",
            "currency": "AUD"
        })
        .to_string();
        let customer = json!({
            "name": "Original customer",
            "email": "customer@example.com",
            "phone": "333",
            "address": "Customer address"
        })
        .to_string();

        let identity = invoice_identity(
            Some(&business),
            Some(&customer),
            &current_settings(),
            None,
            Some((b"frozen-logo".to_vec(), "png".into())),
            Some((b"current-logo".to_vec(), "jpg".into())),
        );
        assert_eq!(identity.business_name, "Original business");
        assert_eq!(identity.currency, "AUD");
        assert!(identity
            .business_lines
            .iter()
            .any(|line| line == "GST: OLD"));
        assert_eq!(identity.customer_lines[0], "Original customer");
        assert_eq!(identity.logo, Some((b"frozen-logo".to_vec(), "png".into())));
    }

    #[test]
    fn tax_summary_is_rendered_per_rate() {
        let raw = json!([
            {"rate_name":"GST","rate_bp":1000,"inclusive":false,"taxable_net":10000,"tax":1000},
            {"rate_name":"GST Free","rate_bp":0,"inclusive":false,"taxable_net":5000,"tax":0}
        ])
        .to_string();
        let rows = document_totals(15000, 1000, 16000, &raw, "AUD");
        assert!(rows.iter().any(|row| row[0].contains("GST (10.00%)")));
        assert!(rows
            .iter()
            .any(|row| row[0] == "Tax total" && row[1] == "A$10.00"));
    }
}
