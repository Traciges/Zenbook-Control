mod app;
mod components;
mod search;
mod services;
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
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    let config = services::config::AppConfig::load();
    rust_i18n::set_locale(&config.language);
    let a = relm4::RelmApp::new("de.guido.asus-hub");
    load_css();
    relm4::adw::StyleManager::default().set_color_scheme(relm4::adw::ColorScheme::PreferDark);
    a.run::<app::AppModel>(());
}
