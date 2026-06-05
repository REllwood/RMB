# Installing RMB

RMB is a desktop app for **Windows** and **macOS**. Download the installer for your OS from the
project's Releases page, then follow the steps below.

## macOS (`.dmg`)

1. Open the `.dmg` and drag **RMB** to Applications.
2. **First launch (unsigned builds):** macOS may say the app "cannot be opened because Apple cannot
   check it for malicious software." This is expected for an unsigned open-source build — it does **not**
   mean anything is wrong.
   - Go to **System Settings → Privacy & Security**, scroll to the message about RMB, and click
     **Open Anyway** (you'll be asked for your password). Launch RMB again and choose **Open**.
3. Signed + notarized builds (when published with an Apple Developer ID) open with no warning.

## Windows (`.exe` / NSIS installer)

1. Run the installer. It installs per-user (no admin needed).
2. **Unsigned builds** trigger Microsoft **SmartScreen** ("Windows protected your PC"). Click
   **More info → Run anyway**. This warning fades as the signed release builds reputation.

## Where your data is stored

A single SQLite database in your user profile:

- **macOS:** `~/Library/Application Support/com.rhysellwood.rmb/rmb.sqlite`
- **Windows:** `%APPDATA%\com.rhysellwood.rmb\rmb.sqlite`

It is **not encrypted** — RMB relies on your operating-system user account (and full-disk encryption like
FileVault / BitLocker if you enable it). Everything stays on your machine; nothing is sent anywhere.

## Backups

- **Settings → Back up…** writes a safe single-file copy (you choose where).
- **Settings → Restore…** validates the chosen file is an intact database, then replaces your data and
  restarts the app. Back up before restoring.

## Build from source

See the README. In short: install Node ≥ 20.19, Rust ≥ 1.96, and the Tauri prerequisites, then
`npm install && npm run tauri build`.
