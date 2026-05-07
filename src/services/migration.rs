// Ayuz - Unofficial Control Center for Asus Laptops
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

use directories::BaseDirs;
use serde::Deserialize;
use std::path::PathBuf;

use super::config::AppConfig;

/// Legacy flat-schema fields that previous Ayuz versions wrote at the top
/// level of `config.json`, before profiles were introduced. Deserialised
/// once on first run after upgrade so a Default profile can be seeded from
/// the user's existing values, then never touched again.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct LegacyAppConfig {
    // Display
    pub color_profile_index: u32,
    pub oled_dc_dimming: Option<u32>,
    pub target_mode_active: bool,
    pub oled_care_pixel_refresh: bool,
    pub oled_care_panel_autohide: bool,
    pub oled_care_transparency: bool,
    // Audio
    pub audio_profile: u32,
    // Keyboard / system
    pub fan_profile: u32,
    pub kbd_brighten_active: bool,
    pub kbd_dim_active: bool,
    pub kbd_timeout_mode: u32,
    pub kbd_timeout_battery_ac_index: u32,
    pub kbd_timeout_battery_only_index: u32,
    pub kbd_brighten_threshold: Option<f64>,
    pub kbd_dim_threshold: Option<f64>,
    pub touchpad_active: Option<bool>,
    pub input_gestures_active: bool,
    pub input_fn_key_locked: bool,
    pub battery_deep_sleep_active: bool,
    pub gpu_mode: u32,
    pub apu_mem: i32,
}

impl LegacyAppConfig {
    /// Reads `~/.config/ayuz/config.json` once and tries to extract any
    /// legacy flat fields. Returns `None` if the file is absent or
    /// unreadable; missing keys default to their type's default.
    pub fn try_load() -> Option<Self> {
        let path = AppConfig::config_dir()?.join("config.json");
        let text = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&text).ok()
    }
}

fn legacy_config_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|d| d.config_dir().join("asus-hub"))
}

/// Returns true if the legacy `~/.config/asus-hub/` directory exists.
pub fn legacy_dir_exists() -> bool {
    legacy_config_dir()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Returns true if a legacy asus-hub config directory exists and the user
/// hasn't previously declined the migration prompt.
pub fn should_prompt() -> bool {
    legacy_dir_exists() && !AppConfig::load().skip_legacy_migration
}

/// Copies `~/.config/asus-hub/config.json` into `~/.config/ayuz/config.json`
/// (overwriting it), then removes the entire `~/.config/asus-hub/` directory.
pub fn perform_migration() -> Result<(), String> {
    let legacy_dir = legacy_config_dir()
        .ok_or_else(|| "Could not determine legacy config directory".to_string())?;

    let legacy_json = legacy_dir.join("config.json");

    if legacy_json.exists() {
        let dest = AppConfig::config_dir()
            .ok_or_else(|| "Could not determine config directory".to_string())?
            .join("config.json");

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {e}"))?;
        }

        std::fs::copy(&legacy_json, &dest)
            .map_err(|e| format!("Failed to copy config.json: {e}"))?;
    }

    std::fs::remove_dir_all(&legacy_dir)
        .map_err(|e| format!("Failed to remove legacy config dir: {e}"))?;

    Ok(())
}
