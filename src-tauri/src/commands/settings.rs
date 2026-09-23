//! Settings + tax-rate commands.

use rmb_data::db::Db;
use rmb_data::repos::settings::{self, Settings, TaxRateRow};
use tauri::State;

use crate::error::AppError;

const MAX_LOGO_BYTES: u64 = 5 * 1024 * 1024;
const MAX_LOGO_DIMENSION: u32 = 8_000;
const MAX_LOGO_PIXELS: u64 = 16_000_000;

#[tauri::command]
pub async fn get_settings(db: State<'_, Db>) -> Result<Settings, AppError> {
    Ok(settings::get(&db).await?)
}

#[tauri::command]
pub async fn update_settings(db: State<'_, Db>, value: Settings) -> Result<(), AppError> {
    settings::update(&db, &value).await?;
    Ok(())
}

#[tauri::command]
pub async fn list_tax_rates(db: State<'_, Db>) -> Result<Vec<TaxRateRow>, AppError> {
    Ok(settings::list_tax_rates(&db, false).await?)
}

#[tauri::command]
pub async fn create_tax_rate(
    db: State<'_, Db>,
    name: String,
    rate_bp: i64,
    inclusive: bool,
) -> Result<i64, AppError> {
    Ok(settings::create_tax_rate(&db, &name, rate_bp, inclusive).await?)
}

#[tauri::command]
pub async fn update_tax_rate(
    db: State<'_, Db>,
    id: i64,
    name: String,
    rate_bp: i64,
    inclusive: bool,
) -> Result<(), AppError> {
    settings::update_tax_rate(&db, id, &name, rate_bp, inclusive).await?;
    Ok(())
}

/// Archived tax rates, for reviewing and restoring.
#[tauri::command]
pub async fn list_archived_tax_rates(db: State<'_, Db>) -> Result<Vec<TaxRateRow>, AppError> {
    Ok(settings::list_tax_rates(&db, true)
        .await?
        .into_iter()
        .filter(|rate| rate.archived)
        .collect())
}

#[tauri::command]
pub async fn restore_tax_rate(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    settings::restore_tax_rate(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn archive_tax_rate(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    settings::archive_tax_rate(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn apply_tax_preset(db: State<'_, Db>, country: String) -> Result<(), AppError> {
    settings::apply_tax_preset(&db, &country).await?;
    Ok(())
}

/// Import a business logo: read the chosen file once (size-capped), check it is a complete PNG or
/// JPEG of sensible dimensions, and store those exact bytes in the database. Issued invoices keep
/// the logo they were issued with, so later changes never alter historical documents. The path is
/// kept only to show which file was chosen.
#[tauri::command]
pub async fn set_logo(db: State<'_, Db>, src: String) -> Result<String, AppError> {
    let src_path = std::path::Path::new(&src)
        .canonicalize()
        .map_err(|e| AppError::Message(format!("cannot read the chosen file: {e}")))?;
    let ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let (expected_format, stored_format) = match ext.as_str() {
        "png" => (image::ImageFormat::Png, "png"),
        "jpg" | "jpeg" => (image::ImageFormat::Jpeg, "jpg"),
        _ => {
            return Err(AppError::Message(
                "the logo must be a PNG or JPEG image".into(),
            ))
        }
    };

    // One read, capped: the bytes checked below are exactly the bytes stored.
    let mut logo_data = Vec::new();
    std::fs::File::open(&src_path)
        .and_then(|file| {
            use std::io::Read;
            file.take(MAX_LOGO_BYTES + 1).read_to_end(&mut logo_data)
        })
        .map_err(|e| AppError::Message(format!("cannot read the chosen file: {e}")))?;
    if logo_data.is_empty() {
        return Err(AppError::Message("the chosen logo is empty".into()));
    }
    if logo_data.len() as u64 > MAX_LOGO_BYTES {
        return Err(AppError::Message("the logo must be under 5 MB".into()));
    }

    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(&logo_data), expected_format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_LOGO_DIMENSION);
    limits.max_image_height = Some(MAX_LOGO_DIMENSION);
    limits.max_alloc = Some(MAX_LOGO_PIXELS * 4);
    reader.limits(limits);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| AppError::Message("the chosen file is not a valid PNG or JPEG".into()))?;
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > MAX_LOGO_DIMENSION
        || height > MAX_LOGO_DIMENSION
        || pixels > MAX_LOGO_PIXELS
    {
        return Err(AppError::Message(format!(
            "logo dimensions must be no more than {MAX_LOGO_DIMENSION} pixels per side and {MAX_LOGO_PIXELS} pixels in total"
        )));
    }
    let mut decoder =
        image::ImageReader::with_format(std::io::Cursor::new(&logo_data), expected_format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_LOGO_PIXELS * 4);
    decoder.limits(limits);
    decoder.decode().map_err(|_| {
        AppError::Message("the chosen file is not a complete PNG or JPEG of that type".into())
    })?;

    let path = src_path.to_string_lossy().into_owned();
    settings::set_logo_asset(&db, Some(&path), Some((&logo_data, stored_format))).await?;
    Ok(path)
}

/// Stop using a logo for future documents. Versioned assets remain because issued invoice and
/// receipt snapshots may still reference them.
#[tauri::command]
pub async fn clear_logo(db: State<'_, Db>) -> Result<(), AppError> {
    settings::set_logo_asset(&db, None, None).await?;
    Ok(())
}
