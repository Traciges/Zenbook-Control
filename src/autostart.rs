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

use directories::BaseDirs;
use std::path::PathBuf;

const DESKTOP_FILENAME: &str = "io.github.traciges.asus-hub.desktop";

fn autostart_path() -> Option<PathBuf> {
    BaseDirs::new().map(|d| d.config_dir().join("autostart").join(DESKTOP_FILENAME))
}

pub fn is_enabled() -> bool {
    autostart_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn set_enabled(enable: bool) {
    let Some(path) = autostart_path() else {
        return;
    };
    if enable {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = "[Desktop Entry]\n\
            Type=Application\n\
            Name=Asus Hub\n\
            Exec=asus-hub --hidden\n\
            Hidden=false\n\
            StartupNotify=false\n\
            X-GNOME-Autostart-enabled=true\n";
        let _ = std::fs::write(&path, content);
    } else {
        let _ = std::fs::remove_file(&path);
    }
}
