// Asus Hub - Unofficial Control Center for Asus Laptops
// Copyright (C) 2026 Guido Philipp
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see https://www.gnu.org/licenses/.

//! Helpers for ICC color profile management and KDE D-Bus utilities.
//!
//! ICC profiles are embedded in the binary at compile time and extracted to
//! `~/.config/asus-hub/icm/` on first use. Profiles are applied via `kscreen-doctor`.

use crate::services::commands::{resolve_qdbus_path, run_command_blocking};
use crate::services::config::AppConfig;
use rust_i18n::t;

/// `kscreen-doctor` output name for the built-in laptop display.
pub(crate) const DISPLAY_NAME: &str = "eDP-1";

const SRGB_ICM: &[u8] = include_bytes!("../../../assets/icm/ASUS_sRGB.icm");
const DCIP3_ICM: &[u8] = include_bytes!("../../../assets/icm/ASUS_DCIP3.icm");
const DISPLAYP3_ICM: &[u8] = include_bytes!("../../../assets/icm/ASUS_DisplayP3.icm");

/// Extracts the bundled ICM files to `~/.config/asus-hub/icm/` and returns that directory path.
///
/// Each file is only written if it does not already exist, making this safe to call on every
/// startup without unnecessary disk writes.
pub(crate) async fn setup_icm_profiles() -> Result<std::path::PathBuf, String> {
    let base = AppConfig::config_dir()
        .ok_or_else(|| t!("error_config_dir").to_string())?
        .join("icm");

    let base_clone = base.clone();
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&base_clone)
            .map_err(|e| t!("error_icm_dir_create", error = e.to_string()).to_string())?;

        for (name, data) in [
            ("ASUS_sRGB.icm", SRGB_ICM),
            ("ASUS_DCIP3.icm", DCIP3_ICM),
            ("ASUS_DisplayP3.icm", DISPLAYP3_ICM),
        ] {
            let path = base_clone.join(name);
            if !path.exists() {
                std::fs::write(&path, data).map_err(|e| {
                    t!("error_icm_write", name = name, error = e.to_string()).to_string()
                })?;
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| t!("error_spawn_blocking", error = e.to_string()).to_string())??;

    Ok(base)
}

/// Resets the display color profile to the monitor's built-in EDID default via `kscreen-doctor`.
pub(crate) async fn reset_icm_profile() -> Result<(), String> {
    let arg = format!("output.{}.colorProfileSource.EDID", DISPLAY_NAME);
    run_command_blocking("kscreen-doctor", &[&arg]).await
}

/// Applies an ICC profile file to [`DISPLAY_NAME`] via `kscreen-doctor`.
///
/// The argument format is `output.<display>.iccprofile.<absolute_path>`.
pub(crate) async fn apply_icm_profile(
    filename: &str,
    base_path: &std::path::Path,
) -> Result<(), String> {
    let arg = format!(
        "output.{}.iccprofile.{}",
        DISPLAY_NAME,
        base_path.join(filename).display()
    );
    run_command_blocking("kscreen-doctor", &[&arg]).await
}

/// Invokes a D-Bus method via the `qdbus` command-line tool.
///
/// The executable path is resolved once via [`resolve_qdbus_path`], which checks `$PATH` first,
/// then falls back to known Arch Linux locations (`/usr/lib/qt6/bin/qdbus`,
/// `/usr/lib/qt5/bin/qdbus`).
pub(crate) async fn run_qdbus(args: Vec<String>) -> Result<(), String> {
    let cmd = resolve_qdbus_path();
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    run_command_blocking(cmd, &args_ref).await
}
