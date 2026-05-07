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

//! Shared helpers for evdev-based listener services (`fan_hotkey`,
//! `edge_gestures`). Device enumeration criteria differ per consumer, but the
//! stream-open + error logging boilerplate is identical.

use evdev::{Device, EventStream};
use rust_i18n::t;

/// Converts a [`Device`] into an [`EventStream`], logging a warning and
/// returning `None` if the conversion fails.
pub fn open_event_stream(device: Device) -> Option<EventStream> {
    match device.into_event_stream() {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!("{}", t!("error_event_read", error = e.to_string()));
            None
        }
    }
}
