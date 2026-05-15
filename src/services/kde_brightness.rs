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

#[zbus::proxy(
    interface = "org.kde.Solid.PowerManagement.Actions.BrightnessControl",
    default_service = "org.kde.Solid.PowerManagement",
    default_path = "/org/kde/Solid/PowerManagement/Actions/BrightnessControl"
)]
pub trait BrightnessControl {
    #[zbus(name = "brightness")]
    fn brightness(&self) -> zbus::Result<i32>;

    #[zbus(name = "brightnessMax")]
    fn brightness_max(&self) -> zbus::Result<i32>;

    #[zbus(name = "setBrightness")]
    fn set_brightness(&self, value: i32) -> zbus::Result<()>;

    #[zbus(signal, name = "brightnessChanged")]
    fn brightness_changed(&self, brightness: i32) -> zbus::Result<()>;
}

/// Adjust the screen brightness by `delta_percent` (signed, e.g. +5 / -5) via KDE PowerDevil.
/// Returns Err if D-Bus is unreachable / PowerDevil is missing; callers should fall back to
/// brightnessctl.
pub async fn adjust_brightness_relative(delta_percent: i32) -> Result<(), String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let proxy = BrightnessControlProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;
    let max = proxy.brightness_max().await.map_err(|e| e.to_string())?;
    let cur = proxy.brightness().await.map_err(|e| e.to_string())?;
    let step = (max as f64 * (delta_percent as f64) / 100.0).round() as i32;
    let next = (cur + step).clamp(0, max);
    proxy.set_brightness(next).await.map_err(|e| e.to_string())
}
