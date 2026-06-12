//! PDF generation via Typst — fully offline, using Typst's embedded default fonts so output is
//! byte-identical across platforms. All money/dates are pre-formatted to strings in Rust.

use typst::foundations::{Bytes, Dict, IntoValue};
use typst_as_lib::TypstEngine;

const TEMPLATE: &str = include_str!("../assets/templates/document.typ");

/// Render the document template to PDF bytes. `logo` is optional raster bytes + Typst format
/// name ("png" / "jpg") for the business logo in the header.
pub fn render(json_data: &str, logo: Option<(Vec<u8>, &str)>) -> Result<Vec<u8>, String> {
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
        .map_err(|e| format!("template error: {e:?}"))?;
    typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).map_err(|e| format!("pdf error: {e:?}"))
}

/// Format integer minor units to a display string for the given currency.
pub fn fmt_money(minor: i64, currency: &str) -> String {
    let symbol = match currency {
        "USD" => "$",
        "GBP" => "£",
        "EUR" => "€",
        "AUD" => "A$",
        "NZD" => "NZ$",
        "CAD" => "C$",
        _ => "",
    };
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    let major = abs / 100;
    let cents = abs % 100;

    let digits = major.to_string();
    let len = digits.len();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }

    if symbol.is_empty() {
        format!("{sign}{grouped}.{cents:02} {currency}")
    } else {
        format!("{sign}{symbol}{grouped}.{cents:02}")
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
        let branded = render(&data, Some((logo, "png"))).expect("logo render should succeed");
        assert!(branded.starts_with(b"%PDF"));
        assert!(branded.len() > pdf.len(), "logo should add content");
    }

    #[test]
    fn money_formatting() {
        assert_eq!(fmt_money(123456, "USD"), "$1,234.56");
        assert_eq!(fmt_money(99, "GBP"), "£0.99");
        assert_eq!(fmt_money(-5000, "EUR"), "-€50.00");
        assert_eq!(fmt_money(100000, "SEK"), "1,000.00 SEK");
    }
}
