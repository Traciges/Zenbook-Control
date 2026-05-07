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

use evdev::{Device, EventSummary, KeyCode};
use rust_i18n::t;
use tokio::sync::watch;

use crate::components::system::fan::FanMsg;
use crate::services::evdev_runner::open_event_stream;

/// Raw evdev keycodes the ASUS fan/profile key may emit, depending on model
/// and kernel version. The kernel maps the same physical key differently
/// across asus-wmi revisions, so we accept any of these.
///
/// - `204` (`KEY_PROG4`) - older asus-nb-wmi mapping.
/// - `425` - historic asus-wmi mapping seen on some ROG firmware revisions.
/// - `467` (`KEY_FN_F5`) - alternative mapping seen on some firmware revisions.
/// - `482` (`KEY_FN_F`) - current asus-wmi mapping on recent kernels.
const FAN_KEYCODES: &[u16] = &[204, 425, 467, 482];

/// Locates the input device most likely to deliver the fan key.
///
/// Preference order:
/// 1. A device whose name (lower-cased) contains "asus" AND "wmi"/"hotkey" -
///    the dedicated `Asus WMI hotkeys` evdev node.
/// 2. Any keyboard device that advertises one of [`FAN_KEYCODES`].
fn find_hotkey_device() -> Option<Device> {
    let mut keyboard_fallback: Option<Device> = None;

    for (_, device) in evdev::enumerate() {
        let name = device.name().unwrap_or_default().to_lowercase();
        let is_asus_hotkeys =
            name.contains("asus") && (name.contains("wmi") || name.contains("hotkey"));

        if let Some(keys) = device.supported_keys() {
            let advertises_fan_key = FAN_KEYCODES.iter().any(|&c| keys.contains(KeyCode::new(c)));

            if is_asus_hotkeys && advertises_fan_key {
                return Some(device);
            }
            if advertises_fan_key && keyboard_fallback.is_none() {
                keyboard_fallback = Some(device);
            }
        }
    }
    keyboard_fallback
}

/// Watches the ASUS fan key and emits [`FanMsg::CycleFromHotkey`] on each press,
/// gated by `enabled_rx` (mirrors the `fan_hotkey_enabled` config flag).
pub async fn run(sender: relm4::Sender<FanMsg>, enabled_rx: watch::Receiver<bool>) {
    let device = match find_hotkey_device() {
        Some(d) => d,
        None => {
            tracing::warn!("{}", t!("error_fan_hotkey_no_device"));
            return;
        }
    };

    if let Some(name) = device.name() {
        tracing::info!("{}", t!("fan_hotkey_listening", device = name));
    }

    let Some(mut stream) = open_event_stream(device) else {
        return;
    };

    loop {
        let event = match stream.next_event().await {
            Ok(ev) => ev,
            Err(e) => {
                tracing::warn!("{}", t!("error_event_read", error = e.to_string()));
                break;
            }
        };

        if let EventSummary::Key(_, key, 1) = event.destructure() {
            if FAN_KEYCODES.contains(&key.code()) && *enabled_rx.borrow() {
                sender.emit(FanMsg::CycleFromHotkey);
            }
        }
    }
}
