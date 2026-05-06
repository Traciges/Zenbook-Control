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

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;

fn default_brighten_threshold() -> f64 {
    12.0
}
fn default_dim_threshold() -> f64 {
    35.0
}
fn default_touchpad_active() -> bool {
    true
}
fn default_dc_dimming() -> u32 {
    100
}
fn default_language() -> String {
    "en".to_string()
}
fn default_profiles() -> Vec<Profile> {
    vec![]
}
fn default_profile_icon() -> String {
    "computer-symbolic".to_string()
}
fn default_volume() -> f64 {
    100.0
}
fn default_aura_brightness() -> u32 {
    2
}
fn default_aura_colour_r() -> u8 {
    166
}
fn default_animatrix_enable() -> bool {
    true
}
fn default_animatrix_brightness() -> u32 {
    2
}
fn default_animatrix_builtins() -> bool {
    true
}
fn default_animatrix_boot_anim() -> String {
    "GlitchConstruction".to_string()
}
fn default_animatrix_awake_anim() -> String {
    "BinaryBannerScroll".to_string()
}
fn default_animatrix_sleep_anim() -> String {
    "BannerSwipe".to_string()
}
fn default_animatrix_shutdown_anim() -> String {
    "GlitchOut".to_string()
}

fn generate_profile_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}{:04x}", t.as_secs(), t.subsec_millis())
}

/// A named hardware + software preset that can be switched at runtime.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default = "default_profile_icon")]
    pub icon: String,

    // Display
    pub fan_profile: u32,
    pub oled_dc_dimming: u32,
    pub target_mode_active: bool,
    pub color_profile_index: u32,
    pub oled_care_pixel_refresh: bool,
    pub oled_care_panel_autohide: bool,
    pub oled_care_transparency: bool,

    // Audio
    pub audio_profile: u32,
    #[serde(default = "default_volume")]
    pub volume: f64,

    // Keyboard & input
    pub kbd_timeout_mode: u32,
    pub kbd_timeout_battery_ac_index: u32,
    pub kbd_timeout_battery_only_index: u32,
    pub kbd_brighten_active: bool,
    pub kbd_dim_active: bool,
    pub kbd_brighten_threshold: f64,
    pub kbd_dim_threshold: f64,
    pub touchpad_active: bool,
    pub input_gestures_active: bool,
    pub input_fn_key_locked: bool,

    // System
    pub battery_deep_sleep_active: bool,
    pub gpu_mode: u32,
    pub apu_mem: i32,

    // Keyboard RGB (Aura)
    #[serde(default)]
    pub aura_mode: u32,
    #[serde(default = "default_aura_brightness")]
    pub aura_brightness: u32,
    #[serde(default = "default_aura_colour_r")]
    pub aura_colour_r: u8,
    #[serde(default)]
    pub aura_colour_g: u8,
    #[serde(default)]
    pub aura_colour_b: u8,

    // AniMatrix LED panel
    #[serde(default = "default_animatrix_enable")]
    pub animatrix_enable_display: bool,
    #[serde(default = "default_animatrix_brightness")]
    pub animatrix_brightness: u32,
    #[serde(default = "default_animatrix_builtins")]
    pub animatrix_builtins_enabled: bool,
    #[serde(default = "default_animatrix_boot_anim")]
    pub animatrix_boot_anim: String,
    #[serde(default = "default_animatrix_awake_anim")]
    pub animatrix_awake_anim: String,
    #[serde(default = "default_animatrix_sleep_anim")]
    pub animatrix_sleep_anim: String,
    #[serde(default = "default_animatrix_shutdown_anim")]
    pub animatrix_shutdown_anim: String,
    #[serde(default)]
    pub animatrix_off_when_unplugged: bool,
    #[serde(default)]
    pub animatrix_off_when_suspended: bool,
    #[serde(default)]
    pub animatrix_off_when_lid_closed: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Default".to_string(),
            icon: "computer-symbolic".to_string(),
            fan_profile: 0,
            oled_dc_dimming: 100,
            target_mode_active: false,
            color_profile_index: 0,
            oled_care_pixel_refresh: false,
            oled_care_panel_autohide: false,
            oled_care_transparency: false,
            audio_profile: 0,
            volume: 100.0,
            kbd_timeout_mode: 0,
            kbd_timeout_battery_ac_index: 0,
            kbd_timeout_battery_only_index: 0,
            kbd_brighten_active: false,
            kbd_dim_active: false,
            kbd_brighten_threshold: 12.0,
            kbd_dim_threshold: 35.0,
            touchpad_active: true,
            input_gestures_active: false,
            input_fn_key_locked: false,
            battery_deep_sleep_active: false,
            gpu_mode: 0,
            apu_mem: 0,
            aura_mode: 0,
            aura_brightness: 2,
            aura_colour_r: 166,
            aura_colour_g: 0,
            aura_colour_b: 0,
            animatrix_enable_display: true,
            animatrix_brightness: 2,
            animatrix_builtins_enabled: true,
            animatrix_boot_anim: "GlitchConstruction".to_string(),
            animatrix_awake_anim: "BinaryBannerScroll".to_string(),
            animatrix_sleep_anim: "BannerSwipe".to_string(),
            animatrix_shutdown_anim: "GlitchOut".to_string(),
            animatrix_off_when_unplugged: false,
            animatrix_off_when_suspended: false,
            animatrix_off_when_lid_closed: false,
        }
    }
}

/// Persistent application configuration stored as JSON at `~/.config/ayuz/config.json`.
#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    // ── Global (non-profile) settings ────────────────────────────────────────
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub skip_legacy_migration: bool,

    // ── Profile management ───────────────────────────────────────────────────
    #[serde(default)]
    pub active_profile_id: String,
    #[serde(default = "default_profiles")]
    pub profiles: Vec<Profile>,

    // ── Legacy fields (read for migration only, never written) ────────────────
    // Display
    #[serde(default, skip_serializing)]
    pub color_profile_index: u32,
    #[serde(default = "default_dc_dimming", skip_serializing)]
    pub oled_dc_dimming: u32,
    #[serde(default, skip_serializing)]
    pub target_mode_active: bool,
    #[serde(default, skip_serializing)]
    pub oled_care_pixel_refresh: bool,
    #[serde(default, skip_serializing)]
    pub oled_care_panel_autohide: bool,
    #[serde(default, skip_serializing)]
    pub oled_care_transparency: bool,
    // Audio
    #[serde(default, skip_serializing)]
    pub audio_profile: u32,
    // Keyboard
    #[serde(default, skip_serializing)]
    pub fan_profile: u32,
    #[serde(default, skip_serializing)]
    pub kbd_brighten_active: bool,
    #[serde(default, skip_serializing)]
    pub kbd_dim_active: bool,
    #[serde(default, skip_serializing)]
    pub kbd_timeout_mode: u32,
    #[serde(default, skip_serializing)]
    pub kbd_timeout_battery_ac_index: u32,
    #[serde(default, skip_serializing)]
    pub kbd_timeout_battery_only_index: u32,
    #[serde(default = "default_brighten_threshold", skip_serializing)]
    pub kbd_brighten_threshold: f64,
    #[serde(default = "default_dim_threshold", skip_serializing)]
    pub kbd_dim_threshold: f64,
    #[serde(default = "default_touchpad_active", skip_serializing)]
    pub touchpad_active: bool,
    #[serde(default, skip_serializing)]
    pub input_gestures_active: bool,
    #[serde(default, skip_serializing)]
    pub input_fn_key_locked: bool,
    // System
    #[serde(default, skip_serializing)]
    pub battery_deep_sleep_active: bool,
    #[serde(default, skip_serializing)]
    pub gpu_mode: u32,
    #[serde(default, skip_serializing)]
    pub apu_mem: i32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            skip_legacy_migration: false,
            active_profile_id: String::new(),
            profiles: vec![],
            // legacy defaults
            color_profile_index: 0,
            oled_dc_dimming: default_dc_dimming(),
            target_mode_active: false,
            oled_care_pixel_refresh: false,
            oled_care_panel_autohide: false,
            oled_care_transparency: false,
            audio_profile: 0,
            fan_profile: 0,
            kbd_brighten_active: false,
            kbd_dim_active: false,
            kbd_timeout_mode: 0,
            kbd_timeout_battery_ac_index: 0,
            kbd_timeout_battery_only_index: 0,
            kbd_brighten_threshold: default_brighten_threshold(),
            kbd_dim_threshold: default_dim_threshold(),
            touchpad_active: default_touchpad_active(),
            input_gestures_active: false,
            input_fn_key_locked: false,
            battery_deep_sleep_active: false,
            gpu_mode: 0,
            apu_mem: 0,
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> Option<std::path::PathBuf> {
        ProjectDirs::from("", "", "ayuz").map(|dirs| dirs.config_dir().to_path_buf())
    }

    fn config_path() -> Option<std::path::PathBuf> {
        Self::config_dir().map(|dir| dir.join("config.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            let mut c = Self::default();
            c.ensure_default_profile();
            return c;
        };
        let mut config: AppConfig = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        config.ensure_default_profile();
        config
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, json);
        }
    }

    pub fn update(f: impl FnOnce(&mut Self)) {
        let mut config = Self::load();
        f(&mut config);
        config.save();
    }

    /// Returns a reference to the currently active profile.
    pub fn active_profile(&self) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_profile_id)
            .or_else(|| self.profiles.first())
            .expect("profiles must not be empty after ensure_default_profile")
    }

    /// Returns a mutable reference to the currently active profile.
    pub fn active_profile_mut(&mut self) -> &mut Profile {
        let id = self.active_profile_id.clone();
        if let Some(idx) = self.profiles.iter().position(|p| p.id == id) {
            return &mut self.profiles[idx];
        }
        &mut self.profiles[0]
    }

    /// If `profiles` is empty (first run / upgrade from legacy config), creates a "Default"
    /// profile seeded from the legacy flat fields and sets it as active.
    pub fn ensure_default_profile(&mut self) {
        if !self.profiles.is_empty() {
            return;
        }
        let id = generate_profile_id();
        self.profiles.push(Profile {
            id: id.clone(),
            name: "Default".to_string(),
            icon: "computer-symbolic".to_string(),
            fan_profile: self.fan_profile,
            oled_dc_dimming: self.oled_dc_dimming,
            target_mode_active: self.target_mode_active,
            color_profile_index: self.color_profile_index,
            oled_care_pixel_refresh: self.oled_care_pixel_refresh,
            oled_care_panel_autohide: self.oled_care_panel_autohide,
            oled_care_transparency: self.oled_care_transparency,
            audio_profile: self.audio_profile,
            volume: 100.0,
            kbd_timeout_mode: self.kbd_timeout_mode,
            kbd_timeout_battery_ac_index: self.kbd_timeout_battery_ac_index,
            kbd_timeout_battery_only_index: self.kbd_timeout_battery_only_index,
            kbd_brighten_active: self.kbd_brighten_active,
            kbd_dim_active: self.kbd_dim_active,
            kbd_brighten_threshold: self.kbd_brighten_threshold,
            kbd_dim_threshold: self.kbd_dim_threshold,
            touchpad_active: self.touchpad_active,
            input_gestures_active: self.input_gestures_active,
            input_fn_key_locked: self.input_fn_key_locked,
            battery_deep_sleep_active: self.battery_deep_sleep_active,
            gpu_mode: self.gpu_mode,
            apu_mem: self.apu_mem,
            aura_mode: 0,
            aura_brightness: 2,
            aura_colour_r: 166,
            aura_colour_g: 0,
            aura_colour_b: 0,
            animatrix_enable_display: true,
            animatrix_brightness: 2,
            animatrix_builtins_enabled: true,
            animatrix_boot_anim: "GlitchConstruction".to_string(),
            animatrix_awake_anim: "BinaryBannerScroll".to_string(),
            animatrix_sleep_anim: "BannerSwipe".to_string(),
            animatrix_shutdown_anim: "GlitchOut".to_string(),
            animatrix_off_when_unplugged: false,
            animatrix_off_when_suspended: false,
            animatrix_off_when_lid_closed: false,
        });
        self.active_profile_id = id;
        self.save();
    }
}
