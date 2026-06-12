//! Settings + tax-rate commands.

use rmb_data::db::Db;
use rmb_data::repos::settings::{self, Settings, TaxRateRow};
use tauri::{Manager, State};

use crate::error::AppError;

const LOGO_NAMES: [&str; 3] = ["logo.png", "logo.jpg", "logo.jpeg"];
const MAX_LOGO_BYTES: u64 = 5 * 1024 * 1024;

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

/// Import a business logo: validate it's a small PNG/JPEG, copy it into the app data dir
/// (so the original can move/disappear), and store the path in settings for PDF branding.
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

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(e.to_string()))?;
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Message(e.to_string()))?;
    for name in LOGO_NAMES {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let dest = dir.join(format!("logo.{ext}"));
    std::fs::copy(&src_path, &dest).map_err(|e| AppError::Message(format!("copy failed: {e}")))?;

    let dest_str = dest.to_string_lossy().into_owned();
    let mut s = settings::get(&db).await?;
    s.logo_path = Some(dest_str.clone());
    settings::update(&db, &s).await?;
    Ok(dest_str)
}

/// Remove the stored logo (file + setting).
#[tauri::command]
pub async fn clear_logo(app: tauri::AppHandle, db: State<'_, Db>) -> Result<(), AppError> {
    if let Ok(dir) = app.path().app_data_dir() {
        for name in LOGO_NAMES {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
    let mut s = settings::get(&db).await?;
    s.logo_path = None;
    settings::update(&db, &s).await?;
    Ok(())
}
