use crate::components::display::splendid::SplendidProfil;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub splendid_profil: SplendidProfil,
    pub farbtemperatur: f64,
    pub eye_care_staerke: f64,
    pub oled_care_pixel_refresh: bool,
    pub oled_care_panel_autohide: bool,
    pub oled_care_transparenz: bool,
    pub fan_tiefschlaf_aktiv: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            splendid_profil: SplendidProfil::Normal,
            farbtemperatur: 4500.0,
            eye_care_staerke: 50.0,
            oled_care_pixel_refresh: false,
            oled_care_panel_autohide: false,
            oled_care_transparenz: false,
            fan_tiefschlaf_aktiv: false,
        }
    }
}

impl AppConfig {
    fn config_path() -> Option<std::path::PathBuf> {
        ProjectDirs::from("", "", "myasus-linux").map(|dirs| dirs.config_dir().join("config.json"))
    }

    pub fn icc_verzeichnis() -> Option<std::path::PathBuf> {
        ProjectDirs::from("", "", "myasus-linux").map(|dirs| dirs.config_dir().join("icc"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
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
}
