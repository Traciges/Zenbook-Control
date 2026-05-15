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

//! Reusable `relm4` widget templates shared across components.

use gtk::prelude::*;
use relm4::gtk;

/// Inline warning label used by every component that surfaces a "daemon
/// missing / requirement unmet" hint inside an `adw::PreferencesGroup`.
///
/// Captures the constant styling (error class, wrap, left-aligned, margins).
/// Call sites still set `set_label` and, where reactive, `#[watch]
/// set_visible` themselves:
///
/// ```ignore
/// #[template]
/// add = &DaemonWarningLabel {
///     #[watch]
///     set_visible: !model.asusd_available,
///     set_label: &t!("asusd_missing_warning"),
/// },
/// ```
#[relm4::widget_template(pub)]
impl relm4::WidgetTemplate for DaemonWarningLabel {
    view! {
        gtk::Label {
            add_css_class: "error",
            set_wrap: true,
            set_xalign: 0.0,
            set_margin_top: 8,
            set_margin_start: 12,
            set_margin_end: 12,
            set_margin_bottom: 4,
        }
    }
}
