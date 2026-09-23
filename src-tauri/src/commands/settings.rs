//! Settings + tax-rate commands.

use rmb_data::db::Db;
use rmb_data::repos::settings::{self, Settings, TaxRateRow};
use tauri::{Manager, State};

use crate::error::AppError;

const MAX_LOGO_BYTES: u64 = 5 * 1024 * 1024;
const MAX_LOGO_DIMENSION: u32 = 8_000;
const MAX_LOGO_PIXELS: u64 = 16_000_000;
static LOGO_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

/// Import a business logo: validate it's a small PNG/JPEG, copy it to an immutable versioned path
/// in the app data directory, and store that path for future PDFs. Issued invoice snapshots retain
/// the path they were issued with, so later logo changes cannot alter historical documents.
#[tauri::command]
pub async fn set_logo(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    src: String,
) -> Result<String, AppError> {
    let src_path = std::path::Path::new(&src)
        .canonicalize()
        .map_err(|e| AppError::Message(format!("cannot read the chosen file: {e}")))?;
    let ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
        return Err(AppError::Message(
            "the logo must be a PNG or JPEG image".into(),
        ));
    }
    let size = std::fs::metadata(&src_path)
        .map_err(|e| AppError::Message(e.to_string()))?
        .len();
    if size > MAX_LOGO_BYTES {
        return Err(AppError::Message("the logo must be under 5 MB".into()));
    }
    if size == 0 {
        return Err(AppError::Message("the chosen logo is empty".into()));
    }

    let reader = image::ImageReader::open(&src_path)
        .map_err(|_| AppError::Message("the chosen file is not a readable image".into()))?
        .with_guessed_format()
        .map_err(|_| AppError::Message("the chosen file is not a readable image".into()))?;
    let expected_format = if ext == "png" {
        image::ImageFormat::Png
    } else {
        image::ImageFormat::Jpeg
    };
    if reader.format() != Some(expected_format) {
        return Err(AppError::Message(
            "the file contents do not match its PNG or JPEG extension".into(),
        ));
    }
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
    let logo_data = std::fs::read(&src_path)
        .map_err(|e| AppError::Message(format!("could not read the validated logo: {e}")))?;
    image::load_from_memory_with_format(&logo_data, expected_format)
        .map_err(|_| AppError::Message("the chosen PNG or JPEG is incomplete or corrupt".into()))?;
    let stored_format = if ext == "png" { "png" } else { "jpg" };

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(e.to_string()))?;
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Message(e.to_string()))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = LOGO_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dest = dir.join(format!(
        "logo-{stamp}-{}-{sequence}.{ext}",
        std::process::id()
    ));
    std::fs::copy(&src_path, &dest).map_err(|e| AppError::Message(format!("copy failed: {e}")))?;

    let dest_str = dest.to_string_lossy().into_owned();
    settings::set_logo_asset(&db, Some(&dest_str), Some((&logo_data, stored_format))).await?;
    Ok(dest_str)
}

/// Stop using a logo for future documents. Versioned assets remain because issued invoice and
/// receipt snapshots may still reference them.
#[tauri::command]
pub async fn clear_logo(db: State<'_, Db>) -> Result<(), AppError> {
    settings::set_logo_asset(&db, None, None).await?;
    Ok(())
}
