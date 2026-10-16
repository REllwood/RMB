//! PDF generation via Typst — fully offline, using Typst's embedded default fonts so output is
//! byte-identical across platforms. All money/dates are pre-formatted to strings in Rust.

use rust_decimal::Decimal;
use typst::foundations::{Bytes, Dict, IntoValue};
use typst_as_lib::TypstEngine;

const TEMPLATE: &str = include_str!("../assets/templates/document.typ");

/// Render the document template to PDF bytes. `logo` is optional raster bytes + Typst format
/// name ("png" / "jpg") for the business logo in the header.
pub fn render(json_data: &str, logo: Option<(Vec<u8>, String)>) -> Result<Vec<u8>, String> {
    let engine = TypstEngine::builder()
        .main_file(TEMPLATE)
        .fonts(typst_assets::fonts())
        .build();

    let mut inputs = Dict::new();
    inputs.insert("data".into(), json_data.into_value());
    if let Some((bytes, format)) = logo {
        inputs.insert("logo".into(), Bytes::new(bytes).into_value());
        inputs.insert("logo_format".into(), format.into_value());
    }

    let doc = engine
        .compile_with_input(inputs)
        .output
        .map_err(|e| format!("the PDF could not be laid out ({})", describe(&e)))?;
    typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).map_err(|diagnostics| {
        format!(
            "the PDF could not be written ({})",
            diagnostics
                .iter()
                .map(|d| d.message.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        )
    })
}

/// A readable summary of a Typst failure (its messages, not the internal debug structure).
fn describe(error: &typst_as_lib::TypstAsLibError) -> String {
    match error {
        typst_as_lib::TypstAsLibError::TypstSource(diagnostics) => diagnostics
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("; "),
        other => other.to_string(),
    }
}

/// Format integer minor units to a display string for the given currency.
pub fn fmt_money(minor: i64, currency: &str) -> String {
    fmt_money_exact(Decimal::from(minor), currency)
}

/// Format an amount in minor units that may carry extra decimal places, such as a unit price
/// before tax (869.7 minor units → `$8.697`). Never fewer than the currency's two decimals.
pub fn fmt_money_exact(minor: Decimal, currency: &str) -> String {
    let symbol = match currency {
        "USD" => "$",
        "GBP" => "£",
        "EUR" => "€",
        "AUD" => "A$",
        "NZD" => "NZ$",
        "CAD" => "C$",
        _ => "",
    };
    let sign = if minor.is_sign_negative() && !minor.is_zero() {
        "-"
    } else {
        ""
    };
    let mut major = (minor.abs() / Decimal::ONE_HUNDRED).normalize();
    if major.scale() < 2 {
        major.rescale(2);
    }
    let text = major.to_string();
    let (digits, fraction) = text.split_once('.').unwrap_or((text.as_str(), "00"));

    let len = digits.len();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }

    if symbol.is_empty() {
        format!("{sign}{grouped}.{fraction} {currency}")
    } else {
        format!("{sign}{symbol}{grouped}.{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_a_pdf() {
        let data = json!({
            "title": "Invoice INV-0001",
            "kind": "INVOICE",
            "business_name": "Acme Plumbing",
            "business_lines": ["1 Pipe St", "acme@example.com · 555-1234", "VAT: GB123456"],
            "meta": ["INV-0001", "Date: 2026-06-05", "Due: 2026-07-05"],
            "customer_block": ["Jane Doe", "2 Tap Road"],
            "lines": [["Labour", "1.50", "$60.00", "$90.00"], ["Cable", "3", "$5.00", "$15.00"]],
            "totals": [["Subtotal", "$105.00"], ["Tax", "$0.00"], ["Total", "$105.00"], ["Balance due", "$105.00"]],
            "notes": "Thank you for your business."
        })
        .to_string();

        let pdf = render(&data, None).expect("render should succeed");
        assert!(pdf.starts_with(b"%PDF"), "output should be a PDF");
        assert!(pdf.len() > 1000, "pdf should have content");

        // With a logo: still a valid PDF, and bigger than the unbranded one (image embedded).
        let logo = include_bytes!("../assets/test-logo.png").to_vec();
        let branded =
            render(&data, Some((logo, "png".into()))).expect("logo render should succeed");
        assert!(branded.starts_with(b"%PDF"));
        assert!(branded.len() > pdf.len(), "logo should add content");
    }

    #[test]
    fn money_formatting() {
        assert_eq!(fmt_money(123456, "USD"), "$1,234.56");
        assert_eq!(fmt_money(99, "GBP"), "£0.99");
        assert_eq!(fmt_money(-5000, "EUR"), "-€50.00");
        assert_eq!(fmt_money(100000, "SEK"), "1,000.00 SEK");
        assert_eq!(fmt_money(0, "USD"), "$0.00");
        assert_eq!(fmt_money(i64::MIN, "USD"), "-$92,233,720,368,547,758.08");
    }

    #[test]
    fn exact_money_keeps_extra_decimals_but_never_fewer_than_two() {
        assert_eq!(fmt_money_exact(Decimal::new(8697, 1), "AUD"), "A$8.697");
        assert_eq!(fmt_money_exact(Decimal::new(-86967, 2), "USD"), "-$8.6967");
        assert_eq!(
            fmt_money_exact(Decimal::new(12345678, 2), "USD"),
            "$1,234.5678"
        );
        assert_eq!(fmt_money_exact(Decimal::new(100000, 2), "GBP"), "£10.00");
        assert_eq!(fmt_money_exact(Decimal::new(5, 1), "SEK"), "0.005 SEK");
    }
}
