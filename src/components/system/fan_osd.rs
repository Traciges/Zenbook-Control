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

use std::cell::RefCell;
use std::time::Duration;

use gtk4 as gtk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use rust_i18n::t;

use crate::services::dbus::FanProfile;

struct OsdState {
    window: gtk::Window,
    label: gtk::Label,
    timeout: Option<glib::SourceId>,
}

thread_local! {
    static OSD: RefCell<Option<OsdState>> = const { RefCell::new(None) };
}

fn profile_label(profile: FanProfile) -> String {
    match profile {
        FanProfile::Performance => t!("fan_performance_title").to_string(),
        FanProfile::Balanced => t!("fan_balanced_title").to_string(),
        FanProfile::Quiet | FanProfile::LowPower => t!("fan_quiet_title").to_string(),
    }
}

fn build() -> OsdState {
    let window = gtk::Window::new();
    window.set_decorated(false);
    window.set_resizable(false);
    window.add_css_class("fan-osd");

    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace(Some("ayuz-osd"));
        window.set_anchor(Edge::Bottom, true);
        window.set_margin(Edge::Bottom, 80);
        window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);
    }

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    hbox.set_margin_top(16);
    hbox.set_margin_bottom(16);
    hbox.set_margin_start(24);
    hbox.set_margin_end(24);

    let icon = gtk::Image::from_icon_name("emblem-system-symbolic");
    icon.set_pixel_size(28);

    let label = gtk::Label::new(None);
    label.add_css_class("fan-osd-label");

    hbox.append(&icon);
    hbox.append(&label);
    window.set_child(Some(&hbox));

    OsdState {
        window,
        label,
        timeout: None,
    }
}

/// Display a transient on-screen indicator showing the active fan profile.
///
/// Auto-dismisses after 1.5 s. Subsequent calls reuse the same window and
/// reset the dismiss timer. The caller is responsible for honouring the
/// `fan_osd_enabled` user preference via `enabled`. No-ops silently when the
/// GTK display is not yet initialised.
pub fn show(profile: FanProfile, enabled: bool) {
    if !enabled {
        return;
    }
    if gtk::gdk::Display::default().is_none() {
        return;
    }

    OSD.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(build());
        }
        let state = slot.as_mut().unwrap();

        state.label.set_label(&profile_label(profile));
        state.window.set_visible(true);
        state.window.present();

        if let Some(id) = state.timeout.take() {
            id.remove();
        }
        state.timeout = Some(glib::timeout_add_local_once(
            Duration::from_millis(1500),
            move || {
                OSD.with(|cell| {
                    if let Some(s) = cell.borrow_mut().as_mut() {
                        s.window.set_visible(false);
                        s.timeout = None;
                    }
                });
            },
        ));
    });
}
