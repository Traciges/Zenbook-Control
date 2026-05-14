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

mod app;
mod autostart;
mod components;
mod search;
mod services;
mod sys_paths;
mod tray;

use gtk4::gdk;

rust_i18n::i18n!("locales", fallback = "en");

const STYLE_CSS: &str = include_str!("../assets/style.css");

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(STYLE_CSS);
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let settings = gtk4::Settings::for_display(&display);
        settings.set_gtk_icon_theme_name(Some("Adwaita"));
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    let config = services::config::AppConfig::load();
    rust_i18n::set_locale(&config.language);

    // GTK4 parses all CLI arguments internally and aborts with "Unknown option"
    // for any flag it doesn't recognize - before our Rust code gets to handle it.
    // We read --hidden ourselves first, then strip it from the args before passing
    // them to GTK via .with_args().
    let args: Vec<String> = std::env::args().collect();

    // Single-instance IPC: if invoked with `--toggle-numberpad`, do not
    // launch a second GTK app. Instead connect to the already-running Ayuz
    // via its abstract Unix socket and request a NumberPad Active/Idle flip.
    if args.iter().any(|arg| arg == "--toggle-numberpad") {
        if let Some(addr) = app::abstract_socket_addr("ayuz-numberpad") {
            use std::io::Write;
            match std::os::unix::net::UnixStream::connect_addr(&addr) {
                Ok(mut s) => {
                    let _ = s.write_all(&[1]);
                }
                Err(e) => {
                    eprintln!("ayuz: --toggle-numberpad: no running instance: {}", e);
                }
            }
        }
        return;
    }

    let start_hidden = args.iter().any(|arg| arg == "--hidden");
    let gtk_args: Vec<String> = args
        .into_iter()
        .filter(|arg| arg != "--hidden" && arg != "--toggle-numberpad")
        .collect();

    gtk4::glib::set_prgname(Some("de.guido.ayuz"));
    let a = relm4::RelmApp::new("de.guido.ayuz").with_args(gtk_args);
    load_css();
    relm4::adw::StyleManager::default().set_color_scheme(relm4::adw::ColorScheme::PreferDark);
    a.run::<app::AppModel>(start_hidden);
}
