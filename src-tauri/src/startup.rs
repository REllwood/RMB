//! Startup: open (or create) the database, run one-time upgrades, take the launch backup, and
//! generate due recurring invoices. Problems that still leave a usable app become warnings on the
//! dashboard; problems that don't are reported to the user in a native dialog instead of a crash.

use std::path::Path;

use rmb_data::db::Db;
use rmb_data::repos::{meta, recurring, settings};

/// Number of most-recent automatic backups kept (older days keep one each; see `prune_auto_backups`).
const LAUNCH_BACKUPS_KEPT: usize = 7;

/// Open everything the app needs. `Err` is a complete, user-facing explanation.
pub async fn initialise(data_dir: &Path) -> Result<Db, String> {
    std::fs::create_dir_all(data_dir).map_err(|error| {
        format!(
            "RMB couldn't create its data folder at {}.\n\n{error}",
            data_dir.display()
        )
    })?;
    let db_path = data_dir.join("rmb.sqlite");
    let backups = data_dir.join("backups");
    let had_backups = contains_backups(&backups);

    let opened = rmb_data::db::open_app(&db_path, &backups)
        .await
        .map_err(|error| fatal_message(&db_path, &backups, &error.to_string()))?;
    let pool = opened.db;
    let mut warnings = Vec::new();

    if opened.created && had_backups {
        warnings.push(format!(
            "RMB couldn't find its database, so it started with an empty one. If you had data, restore \
             it in Settings → Backup & restore. Automatic backups are in {}.",
            backups.display()
        ));
    }
    match meta::take_upgrade_notices(&pool).await {
        Ok(notices) => warnings.extend(notices),
        Err(error) => eprintln!("could not read upgrade notices: {error}"),
    }

    embed_legacy_logo(&pool, &mut warnings).await;
    if let Err(error) = settings::backfill_legacy_invoice_logo_assets(&pool).await {
        warnings.push(format!(
            "Historical invoice logos could not be upgraded. Re-import the logo and restart RMB. ({error})"
        ));
    }

    // Rotating safety net: snapshot the database on every launch.
    if let Err(error) = rmb_data::backup::auto_backup(&pool, &backups, LAUNCH_BACKUPS_KEPT).await {
        eprintln!("auto-backup failed: {error}");
        warnings.push(format!(
            "Automatic backup failed. Create a manual backup in Settings before entering new work. ({error})"
        ));
    }

    // Generate any recurring invoices that came due while the app was closed. Drafts only —
    // nothing is issued without the user.
    match recurring::run_due_now(&pool).await {
        Ok(report) => warnings.extend(report.problems),
        Err(error) => {
            eprintln!("recurring generation failed: {error}");
            warnings.push(format!(
                "Recurring invoices could not be generated. Review Recurring invoices and try Generate due now. ({error})"
            ));
        }
    }

    if let Err(error) = meta::set(&pool, "startup.warning", &warnings.join("\n")).await {
        eprintln!("could not store startup warning state: {error}");
    }
    Ok(pool)
}

fn contains_backups(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".sqlite"))
        })
    })
}

fn fatal_message(db_path: &Path, backups: &Path, error: &str) -> String {
    format!(
        "RMB couldn't open your business data, so it has not changed anything.\n\n\
         {error}\n\n\
         Your database: {}\nBackups: {}\n\n\
         If this started after an update, reinstall the previous version or contact support with \
         this message. Don't delete these files.",
        db_path.display(),
        backups.display()
    )
}

/// One-time upgrade for installs from before logos were embedded: copy the path-only logo into
/// SQLite so backups are portable. Only a file that fully decodes as the image type its extension
/// claims is embedded, so a bad file can never freeze onto issued invoices and break their PDFs.
async fn embed_legacy_logo(pool: &Db, warnings: &mut Vec<String>) {
    match settings::get_logo_asset(pool).await {
        Ok(Some(_)) => {}
        Err(error) => warnings.push(format!(
            "The stored business logo needs attention. Re-import it in Settings. ({error})"
        )),
        Ok(None) => {
            let Ok(current) = settings::get(pool).await else {
                return;
            };
            let Some(path) = current.logo_path.as_deref() else {
                return;
            };
            match read_decodable_logo(Path::new(path)) {
                Some((bytes, format)) => {
                    if let Err(error) =
                        settings::set_logo_asset(pool, Some(path), Some((&bytes, format))).await
                    {
                        warnings.push(format!(
                            "The existing business logo could not be embedded in backups. Re-import it in Settings. ({error})"
                        ));
                    }
                }
                None => warnings.push(
                    "The existing business logo could not be read as a PNG or JPEG. Re-import it in Settings."
                        .into(),
                ),
            }
        }
    }
}

fn read_decodable_logo(path: &Path) -> Option<(Vec<u8>, &'static str)> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let (format, image_format) = match extension.as_str() {
        "png" => ("png", image::ImageFormat::Png),
        "jpg" | "jpeg" => ("jpg", image::ImageFormat::Jpeg),
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() || image::load_from_memory_with_format(&bytes, image_format).is_err() {
        return None;
    }
    Some((bytes, format))
}
