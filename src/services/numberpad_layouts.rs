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

//! Static, proportional NumberPad grid layouts.
//!
//! Each [`Layout`] describes a `rows x cols` grid of touch cells, row-major.
//! The numberpad service slices the touchpad's `ABS_MT_POSITION_X/Y` range
//! into equal-sized rectangles and maps the cell under a tap to a Linux
//! key code (`KEY_KP0`..`KEY_KPENTER`, etc.) emitted via uinput.
//!
//! This is deliberately model-agnostic. The upstream Python driver ships
//! ~30 layout files with hardcoded pixel offsets per laptop; we instead
//! provide a single universal 4x4 grid and a DMI-matched override table
//! for laptops where the universal layout demonstrably misbehaves. Adding
//! a new override is purely additive - no architectural change required.

use evdev::KeyCode;

/// A single cell in the proportional grid. `keys` is a press-and-release
/// sequence — single-key cells use a one-element slice, while macro cells
/// (e.g. `Shift + 5` for `%`) chain multiple codes that the emitter fires
/// in order forward and releases in reverse.
#[derive(Copy, Clone)]
pub struct Cell {
    pub keys: &'static [KeyCode],
}

/// A row-major proportional layout. `cells.len()` must equal `rows * cols`.
/// `None` entries are inert (no key emitted on tap).
pub struct Layout {
    pub rows: u8,
    pub cols: u8,
    pub cells: &'static [Option<Cell>],
}

macro_rules! k {
    ($code:ident) => {
        Some(Cell {
            keys: &[KeyCode::$code],
        })
    };
}

macro_rules! m {
    ($($code:ident),+) => {
        Some(Cell {
            keys: &[$(KeyCode::$code),+],
        })
    };
}

/// Default 4x4 layout, used when the laptop is unknown or no override
/// applies. Mirrors a standard physical numpad.
pub const UNIVERSAL_4X4: Layout = Layout {
    rows: 4,
    cols: 4,
    #[rustfmt::skip]
    cells: &[
        k!(KEY_KP7), k!(KEY_KP8),  k!(KEY_KP9),      k!(KEY_BACKSPACE),
        k!(KEY_KP4), k!(KEY_KP5),  k!(KEY_KP6),      k!(KEY_KPASTERISK),
        k!(KEY_KP1), k!(KEY_KP2),  k!(KEY_KP3),      k!(KEY_KPMINUS),
        k!(KEY_KP0), k!(KEY_KPDOT), k!(KEY_KPENTER), k!(KEY_KPPLUS),
    ],
};

/// ASUS Zenbook 14 (UX3405MA) 5x4 NumberPad. Five columns: the rightmost
/// adds `/`, `*`, `-`, `+` as a dedicated operator column and a tall
/// `Backspace` zone (stacked over rows 0+1). The `%` cell is emitted as a
/// `Shift + 5` macro because Linux has no `KEY_KPPERCENT` evdev code; this
/// produces `%` under any keyboard layout that follows the standard top-row
/// digit mapping (US, DE, LT, ...).
pub const UX3405MA_5X4: Layout = Layout {
    rows: 4,
    cols: 5,
    #[rustfmt::skip]
    cells: &[
        k!(KEY_KP7), k!(KEY_KP8),   k!(KEY_KP9),     k!(KEY_KPSLASH),    k!(KEY_BACKSPACE),
        k!(KEY_KP4), k!(KEY_KP5),   k!(KEY_KP6),     k!(KEY_KPASTERISK), k!(KEY_BACKSPACE),
        k!(KEY_KP1), k!(KEY_KP2),   k!(KEY_KP3),     k!(KEY_KPMINUS),    m!(KEY_LEFTSHIFT, KEY_5),
        k!(KEY_KP0), k!(KEY_KPDOT), k!(KEY_KPENTER), k!(KEY_KPPLUS),     k!(KEY_KPEQUAL),
    ],
};

/// Override table: substring of `/sys/class/dmi/id/product_name` -> layout.
/// First match wins. Add entries here as users report model-specific quirks.
const LAYOUTS: &[(&str, &Layout)] = &[("UX3405MA", &UX3405MA_5X4)];

/// Returns the layout for the given DMI product name, falling back to
/// [`UNIVERSAL_4X4`] when no override matches.
pub fn for_product(product_name: &str) -> &'static Layout {
    LAYOUTS
        .iter()
        .find(|(needle, _)| product_name.contains(needle))
        .map(|(_, l)| *l)
        .unwrap_or(&UNIVERSAL_4X4)
}
