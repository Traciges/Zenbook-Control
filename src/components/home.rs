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

use gtk4 as gtk;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::services::commands::pkexec_read_file;
use crate::services::config::{AppConfig, Profile};
use crate::sys_paths::*;

const IMG_ZENBOOK: &[u8] = include_bytes!("../../assets/img/zenbook.png");
const IMG_VIVOBOOK: &[u8] = include_bytes!("../../assets/img/vivobook.png");
const IMG_TUF: &[u8] = include_bytes!("../../assets/img/tuf.png");
const IMG_ROG: &[u8] = include_bytes!("../../assets/img/rog.png");
const IMG_PROART: &[u8] = include_bytes!("../../assets/img/proart.png");

static PROFILE_CSS: OnceLock<()> = OnceLock::new();

fn ensure_profile_css() {
    PROFILE_CSS.get_or_init(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            "button.profile-active { \
                border: 2px solid @accent_color; \
                border-radius: 12px; \
            }",
        );
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

const PROFILE_ICONS: &[&str] = &[
    "computer-symbolic",
    "starred-symbolic",
    "system-shutdown-symbolic",
    "display-brightness-symbolic",
    "audio-headphones-symbolic",
    "input-gaming-symbolic",
    "power-profile-performance-symbolic",
    "power-profile-balanced-symbolic",
    "battery-good-charging-symbolic",
    "preferences-system-symbolic",
    "system-run-symbolic",
    "emblem-favorite-symbolic",
];

pub struct HomeModel {
    product_name_label: gtk::Label,
    laptop_image: gtk::Picture,
    board_row: adw::ActionRow,
    bios_row: adw::ActionRow,
    kernel_row: adw::ActionRow,
    serial_row: adw::ActionRow,
    reveal_button: gtk::Button,
    metrics_box: gtk::Box,
    battery_label: gtk::Label,
    cpu_label: gtk::Label,
    ram_label: gtk::Label,
    disk_label: gtk::Label,
    profiles: Vec<Profile>,
    active_profile_id: String,
    profiles_flow: gtk::FlowBox,
    profiles_section: gtk::Box,
}

#[derive(Debug)]
pub enum HomeOutput {
    Error(String),
    ActivateProfile(String),
}

#[derive(Debug)]
pub enum HomeMsg {
    RevealSerial,
    ActivateProfile(String),
    CreateProfile,
    RenameProfile { id: String, current_name: String },
    ConfirmRename { id: String, name: String },
    ChangeProfileIcon { id: String },
    ConfirmIconChange { id: String, icon: String },
    DeleteProfile(String),
    RefreshProfiles,
}

#[derive(Debug)]
pub enum HomeCommandOutput {
    DataLoaded {
        product_name: String,
        board_name: String,
        bios_version: String,
        bios_date: String,
        kernel: String,
    },
    SerialRevealed(Result<String, String>),
    MetricsRefreshed {
        battery: String,
        cpu: String,
        ram: String,
        disk: String,
    },
}

fn metric_card(icon_name: &str, title: &str) -> (gtk::Box, gtk::Label) {
    let value_label = gtk::Label::builder()
        .css_classes(["title-2", "dim-label"])
        .halign(gtk::Align::Start)
        .label("…")
        .build();

    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(16);
    let title_label = gtk::Label::new(Some(title));

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.append(&icon);
    header.append(&title_label);

    let inner = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    inner.append(&header);
    inner.append(&value_label);

    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Start)
        .build();
    card.add_css_class("card");
    card.append(&inner);

    (card, value_label)
}

async fn fetch_metrics() -> HomeCommandOutput {
    let battery = {
        let b0 = tokio::fs::read_to_string(SYS_BATTERY0_CAPACITY)
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok());
        let b1 = tokio::fs::read_to_string(SYS_BATTERY1_CAPACITY)
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok());
        match (b0, b1) {
            (Some(a), Some(b)) => format!("{}%", (a as u16 + b as u16) / 2),
            (Some(a), None) | (None, Some(a)) => format!("{}%", a),
            (None, None) => "N/A".to_string(),
        }
    };

    let cpu = {
        let load = tokio::fs::read_to_string(SYS_LOAD_AVG)
            .await
            .map(|s| s.split_whitespace().next().unwrap_or("?").to_string())
            .unwrap_or_else(|_| "?".to_string());

        let temp = tokio::fs::read_to_string(SYS_THERMAL_ZONE0_TEMP)
            .await
            .map(|s| {
                let millideg: i32 = s.trim().parse().unwrap_or(0);
                format!("{}°C", millideg / 1000)
            })
            .unwrap_or_else(|_| "?°C".to_string());

        format!("{}% | {}", load, temp)
    };

    let ram = tokio::fs::read_to_string(SYS_MEM_INFO)
        .await
        .map(|s| {
            let mut total: u64 = 0;
            let mut available: u64 = 0;
            for line in s.lines() {
                if line.starts_with("MemTotal:") {
                    total = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("MemAvailable:") {
                    available = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                }
                if total > 0 && available > 0 {
                    break;
                }
            }
            if total > 0 && available <= total {
                format!("{}%", (total - available) * 100 / total)
            } else {
                "N/A".to_string()
            }
        })
        .unwrap_or_else(|_| "N/A".to_string());

    let disk = tokio::process::Command::new("df")
        .args(["-h", "/"])
        .output()
        .await
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout
                .lines()
                .nth(1)
                .and_then(|line| line.split_whitespace().nth(4))
                .map(|s| s.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        })
        .unwrap_or_else(|_| "N/A".to_string());

    HomeCommandOutput::MetricsRefreshed {
        battery,
        cpu,
        ram,
        disk,
    }
}

fn build_profile_card(
    profile: &Profile,
    active_id: &str,
    sender: &ComponentSender<HomeModel>,
) -> gtk::Widget {
    let is_active = profile.id == active_id;
    let id = profile.id.clone();
    let name = profile.name.clone();
    let is_default = profile.name == "Default";

    // Main button (card)
    let icon = gtk::Image::from_icon_name(&profile.icon);
    icon.set_pixel_size(48);
    icon.set_halign(gtk::Align::Center);

    let name_label = gtk::Label::new(Some(&name));
    name_label.set_halign(gtk::Align::Center);
    name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    name_label.set_max_width_chars(10);

    let inner = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    inner.append(&icon);
    inner.append(&name_label);

    ensure_profile_css();

    let card_btn = gtk::Button::new();
    card_btn.set_child(Some(&inner));
    card_btn.add_css_class("card");
    card_btn.add_css_class("flat");
    if is_active {
        card_btn.add_css_class("accent");
        card_btn.add_css_class("profile-active");
    }

    {
        let sender = sender.clone();
        let id = id.clone();
        card_btn.connect_clicked(move |_| {
            sender.input(HomeMsg::ActivateProfile(id.clone()));
        });
    }

    // Active indicator badge (bottom-left)
    let badge = gtk::Image::from_icon_name("object-select-symbolic");
    badge.set_pixel_size(16);
    badge.add_css_class("accent");
    badge.set_valign(gtk::Align::End);
    badge.set_halign(gtk::Align::Start);
    badge.set_margin_start(8);
    badge.set_margin_bottom(8);
    badge.set_visible(is_active);

    // Menu popover contents
    let rename_btn = gtk::Button::with_label(&t!("home_profile_rename"));
    rename_btn.add_css_class("flat");
    rename_btn.set_halign(gtk::Align::Fill);

    let icon_btn = gtk::Button::with_label(&t!("home_profile_change_icon"));
    icon_btn.add_css_class("flat");
    icon_btn.set_halign(gtk::Align::Fill);

    let delete_btn = gtk::Button::with_label(&t!("home_profile_delete"));
    delete_btn.add_css_class("flat");
    delete_btn.add_css_class("destructive-action");
    delete_btn.set_halign(gtk::Align::Fill);
    delete_btn.set_sensitive(!is_default);

    let menu_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(4)
        .margin_bottom(4)
        .margin_start(4)
        .margin_end(4)
        .build();
    menu_box.append(&rename_btn);
    menu_box.append(&icon_btn);
    menu_box.append(&delete_btn);

    let popover = gtk::Popover::new();
    popover.set_child(Some(&menu_box));

    let menu_button = gtk::MenuButton::new();
    menu_button.set_icon_name("view-more-symbolic");
    menu_button.add_css_class("circular");
    menu_button.add_css_class("flat");
    menu_button.set_popover(Some(&popover));
    menu_button.set_valign(gtk::Align::Start);
    menu_button.set_halign(gtk::Align::End);

    {
        let sender = sender.clone();
        let id = id.clone();
        let name = profile.name.clone();
        rename_btn.connect_clicked(move |_| {
            sender.input(HomeMsg::RenameProfile {
                id: id.clone(),
                current_name: name.clone(),
            });
        });
    }
    {
        let sender = sender.clone();
        let id = id.clone();
        icon_btn.connect_clicked(move |_| {
            sender.input(HomeMsg::ChangeProfileIcon { id: id.clone() });
        });
    }
    {
        let sender = sender.clone();
        let id = id.clone();
        delete_btn.connect_clicked(move |_| {
            sender.input(HomeMsg::DeleteProfile(id.clone()));
        });
    }

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&card_btn));
    overlay.add_overlay(&badge);
    overlay.add_overlay(&menu_button);

    overlay.upcast()
}

fn build_add_profile_card(sender: &ComponentSender<HomeModel>) -> gtk::Widget {
    let icon = gtk::Image::from_icon_name("list-add-symbolic");
    icon.set_pixel_size(32);

    let inner = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    inner.append(&icon);

    let btn = gtk::Button::new();
    btn.set_child(Some(&inner));
    btn.add_css_class("card");
    btn.add_css_class("flat");

    {
        let sender = sender.clone();
        btn.connect_clicked(move |_| {
            sender.input(HomeMsg::CreateProfile);
        });
    }

    btn.upcast()
}

#[relm4::component(pub)]
impl Component for HomeModel {
    type Init = ();
    type Input = HomeMsg;
    type Output = HomeOutput;
    type CommandOutput = HomeCommandOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 24,
            set_margin_top: 24,
            set_margin_bottom: 32,
            set_margin_start: 32,
            set_margin_end: 32,

            append = &adw::PreferencesGroup {
                add = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 24,

                    append = &model.product_name_label.clone(),

                    append = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 32,

                        append = &model.laptop_image.clone(),

                        append = &adw::PreferencesGroup {
                            set_valign: gtk::Align::Center,
                            set_hexpand: true,

                            add = &model.board_row.clone(),
                            add = &model.bios_row.clone(),
                            add = &model.kernel_row.clone(),
                            add = &model.serial_row.clone(),
                        },
                    },
                },
            },

            append = &model.metrics_box.clone(),
            append = &model.profiles_section.clone(),
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let product_name_label = gtk::Label::new(Some(&t!("home_loading")));
        product_name_label.add_css_class("title-1");
        product_name_label.set_halign(gtk::Align::Start);

        let laptop_image = gtk::Picture::new();
        laptop_image.set_width_request(300);
        laptop_image.set_height_request(200);
        laptop_image.set_can_shrink(true);
        laptop_image.set_content_fit(gtk::ContentFit::Contain);
        laptop_image.set_valign(gtk::Align::Center);
        if let Some(display) = gtk::gdk::Display::default() {
            let theme = gtk::IconTheme::for_display(&display);
            let icon = theme.lookup_icon(
                "computer-symbolic",
                &[],
                192,
                1,
                gtk::TextDirection::None,
                gtk::IconLookupFlags::empty(),
            );
            laptop_image.set_paintable(Some(&icon));
        }

        let board_row = adw::ActionRow::new();
        board_row.set_title(&t!("home_board_title"));
        board_row.set_selectable(false);

        let bios_row = adw::ActionRow::new();
        bios_row.set_title(&t!("home_bios_title"));
        bios_row.set_selectable(false);

        let kernel_row = adw::ActionRow::new();
        kernel_row.set_title(&t!("home_kernel_title"));
        kernel_row.set_selectable(false);

        let serial_row = adw::ActionRow::new();
        serial_row.set_title(&t!("home_serial_title"));
        serial_row.set_subtitle(&t!("home_serial_hidden"));
        serial_row.set_selectable(false);

        let reveal_button = gtk::Button::with_label(&t!("home_serial_reveal"));
        reveal_button.set_valign(gtk::Align::Center);
        reveal_button.add_css_class("flat");
        {
            let sender = sender.clone();
            reveal_button.connect_clicked(move |_| {
                sender.input(HomeMsg::RevealSerial);
            });
        }
        serial_row.add_suffix(&reveal_button);

        let (battery_card, battery_label) =
            metric_card("battery-symbolic", &t!("home_metric_battery"));
        let (cpu_card, cpu_label) = metric_card("system-run-symbolic", &t!("home_metric_cpu"));
        let (ram_card, ram_label) = metric_card("media-flash-symbolic", &t!("home_metric_ram"));
        let (disk_card, disk_label) =
            metric_card("drive-harddisk-symbolic", &t!("home_metric_disk"));

        let metrics_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .homogeneous(true)
            .build();
        metrics_box.append(&battery_card);
        metrics_box.append(&cpu_card);
        metrics_box.append(&ram_card);
        metrics_box.append(&disk_card);

        // Profiles section
        let config = AppConfig::load();
        let profiles = config.profiles.clone();
        let active_profile_id = config.active_profile_id.clone();

        let profiles_flow = gtk::FlowBox::new();
        profiles_flow.set_orientation(gtk::Orientation::Horizontal);
        profiles_flow.set_homogeneous(false);
        profiles_flow.set_selection_mode(gtk::SelectionMode::None);
        profiles_flow.set_column_spacing(12);
        profiles_flow.set_row_spacing(12);

        let profiles_header = gtk::Label::new(Some(&t!("home_profiles_title")));
        profiles_header.set_halign(gtk::Align::Start);
        profiles_header.add_css_class("heading");

        let profiles_subtitle = gtk::Label::new(Some(&t!("home_profiles_subtitle")));
        profiles_subtitle.set_halign(gtk::Align::Start);
        profiles_subtitle.set_wrap(true);
        profiles_subtitle.add_css_class("dim-label");

        let profiles_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .height_request(160)
            .child(&profiles_flow)
            .build();

        let profiles_section = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();
        profiles_section.append(&profiles_header);
        profiles_section.append(&profiles_subtitle);
        profiles_section.append(&profiles_scroll);

        let model = HomeModel {
            product_name_label,
            laptop_image,
            board_row,
            bios_row,
            kernel_row,
            serial_row,
            reveal_button,
            metrics_box,
            battery_label,
            cpu_label,
            ram_label,
            disk_label,
            profiles,
            active_profile_id,
            profiles_flow,
            profiles_section,
        };

        let widgets = view_output!();

        // Populate profile cards
        model.rebuild_profile_cards(&sender);

        // Load device info
        sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    let product_name = tokio::fs::read_to_string(SYS_PRODUCT_NAME)
                        .await
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    let board_name = tokio::fs::read_to_string(SYS_BOARD_NAME)
                        .await
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    let bios_version = tokio::fs::read_to_string(SYS_BIOS_VERSION)
                        .await
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    let bios_date = tokio::fs::read_to_string(SYS_BIOS_DATE)
                        .await
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    let kernel = tokio::process::Command::new("uname")
                        .arg("-r")
                        .output()
                        .await
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_default();

                    out.emit(HomeCommandOutput::DataLoaded {
                        product_name,
                        board_name,
                        bios_version,
                        bios_date,
                        kernel,
                    });
                })
                .drop_on_shutdown()
        });

        // Fetch metrics immediately, then every 5 seconds, cancelled on shutdown.
        sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    loop {
                        out.emit(fetch_metrics().await);
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: HomeMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            HomeMsg::RevealSerial => {
                self.reveal_button.set_sensitive(false);
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            let result = pkexec_read_file(SYS_PRODUCT_SERIAL).await;
                            out.emit(HomeCommandOutput::SerialRevealed(result));
                        })
                        .drop_on_shutdown()
                });
            }

            HomeMsg::ActivateProfile(id) => {
                self.active_profile_id = id.clone();
                AppConfig::update(|c| c.active_profile_id = id.clone());
                self.rebuild_profile_cards(&sender);
                let _ = sender.output(HomeOutput::ActivateProfile(id));
            }

            HomeMsg::RefreshProfiles => {
                let config = AppConfig::load();
                self.profiles = config.profiles.clone();
                self.active_profile_id = config.active_profile_id.clone();
                self.rebuild_profile_cards(&sender);
            }

            HomeMsg::CreateProfile => {
                let entry = gtk::Entry::new();
                entry.set_placeholder_text(Some(&t!("home_profile_name_placeholder")));

                let dialog = adw::AlertDialog::builder()
                    .heading(t!("home_profile_create_heading").as_ref())
                    .build();
                dialog.set_extra_child(Some(&entry));
                dialog.add_response("cancel", &t!("dialog_cancel"));
                dialog.add_response("create", &t!("home_profile_create_confirm"));
                dialog.set_default_response(Some("create"));
                dialog.set_close_response("cancel");

                let sender_clone = sender.clone();
                let entry_clone = entry.clone();
                glib::spawn_future_local(async move {
                    let response = dialog.choose_future(None::<&gtk::Widget>).await;
                    if &*response == "create" {
                        let name = entry_clone.text().to_string();
                        if name.is_empty() {
                            return;
                        }
                        AppConfig::update(|c| {
                            let new_profile = {
                                let active = c.active_profile().clone();
                                use std::time::{SystemTime, UNIX_EPOCH};
                                let t = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default();
                                let id = format!("{:x}{:04x}", t.as_secs(), t.subsec_millis());
                                crate::services::config::Profile {
                                    id: id.clone(),
                                    name,
                                    icon: "star-symbolic".to_string(),
                                    ..active
                                }
                            };
                            c.active_profile_id = new_profile.id.clone();
                            c.profiles.push(new_profile);
                        });
                        sender_clone.input(HomeMsg::RefreshProfiles);
                    }
                });
            }

            HomeMsg::RenameProfile { id, current_name } => {
                let entry = gtk::Entry::new();
                entry.set_text(&current_name);
                entry.select_region(0, -1);

                let dialog = adw::AlertDialog::builder()
                    .heading(t!("home_profile_rename_heading").as_ref())
                    .build();
                dialog.set_extra_child(Some(&entry));
                dialog.add_response("cancel", &t!("dialog_cancel"));
                dialog.add_response("confirm", &t!("home_profile_rename_confirm"));
                dialog.set_default_response(Some("confirm"));
                dialog.set_close_response("cancel");

                let sender_clone = sender.clone();
                let entry_clone = entry.clone();
                glib::spawn_future_local(async move {
                    let response = dialog.choose_future(None::<&gtk::Widget>).await;
                    if &*response == "confirm" {
                        let name = entry_clone.text().to_string();
                        if !name.is_empty() {
                            sender_clone.input(HomeMsg::ConfirmRename { id, name });
                        }
                    }
                });
            }

            HomeMsg::ConfirmRename { id, name } => {
                AppConfig::update(|c| {
                    if let Some(p) = c.profiles.iter_mut().find(|p| p.id == id) {
                        p.name = name;
                    }
                });
                sender.input(HomeMsg::RefreshProfiles);
            }

            HomeMsg::ChangeProfileIcon { id } => {
                let selected_icon: Rc<RefCell<String>> =
                    Rc::new(RefCell::new("computer-symbolic".to_string()));

                let flow = gtk::FlowBox::new();
                flow.set_selection_mode(gtk::SelectionMode::None);
                flow.set_max_children_per_line(4);
                flow.set_column_spacing(8);
                flow.set_row_spacing(8);
                flow.set_margin_top(8);
                flow.set_margin_bottom(8);

                for &icon_name in PROFILE_ICONS {
                    let icon_img = gtk::Image::from_icon_name(icon_name);
                    icon_img.set_pixel_size(32);
                    let icon_btn = gtk::Button::new();
                    icon_btn.set_child(Some(&icon_img));
                    icon_btn.add_css_class("flat");

                    let selected = selected_icon.clone();
                    icon_btn.connect_clicked(move |_| {
                        *selected.borrow_mut() = icon_name.to_string();
                    });
                    flow.append(&icon_btn);
                }

                let dialog = adw::AlertDialog::builder()
                    .heading(t!("home_profile_icon_heading").as_ref())
                    .build();
                dialog.set_extra_child(Some(&flow));
                dialog.add_response("cancel", &t!("dialog_cancel"));
                dialog.add_response("confirm", &t!("home_profile_icon_apply"));
                dialog.set_default_response(Some("confirm"));
                dialog.set_close_response("cancel");

                let sender_clone = sender.clone();
                glib::spawn_future_local(async move {
                    let response = dialog.choose_future(None::<&gtk::Widget>).await;
                    if &*response == "confirm" {
                        let icon = selected_icon.borrow().clone();
                        sender_clone.input(HomeMsg::ConfirmIconChange { id, icon });
                    }
                });
            }

            HomeMsg::ConfirmIconChange { id, icon } => {
                AppConfig::update(|c| {
                    if let Some(p) = c.profiles.iter_mut().find(|p| p.id == id) {
                        p.icon = icon;
                    }
                });
                sender.input(HomeMsg::RefreshProfiles);
            }

            HomeMsg::DeleteProfile(id) => {
                let dialog = adw::AlertDialog::builder()
                    .heading(t!("home_profile_delete_heading").as_ref())
                    .body(t!("home_profile_delete_body").as_ref())
                    .build();
                dialog.add_response("cancel", &t!("dialog_cancel"));
                dialog.add_response("delete", &t!("home_profile_delete_confirm"));
                dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
                dialog.set_close_response("cancel");

                let sender_clone = sender.clone();
                glib::spawn_future_local(async move {
                    let response = dialog.choose_future(None::<&gtk::Widget>).await;
                    if &*response == "delete" {
                        AppConfig::update(|c| {
                            c.profiles.retain(|p| p.id != id);
                            if c.active_profile_id == id {
                                if let Some(first) = c.profiles.first() {
                                    c.active_profile_id = first.id.clone();
                                }
                            }
                        });
                        sender_clone.input(HomeMsg::RefreshProfiles);
                    }
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: HomeCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            HomeCommandOutput::DataLoaded {
                product_name,
                board_name,
                bios_version,
                bios_date,
                kernel,
            } => {
                self.product_name_label.set_label(&product_name);

                let name = product_name.to_lowercase();
                let img_bytes: Option<&[u8]> = if name.contains("zenbook") {
                    Some(IMG_ZENBOOK)
                } else if name.contains("vivobook") {
                    Some(IMG_VIVOBOOK)
                } else if name.contains("tuf") {
                    Some(IMG_TUF)
                } else if name.contains("rog") {
                    Some(IMG_ROG)
                } else if name.contains("proart") {
                    Some(IMG_PROART)
                } else {
                    None
                };
                if let Some(bytes) = img_bytes {
                    let bytes = glib::Bytes::from_static(bytes);
                    if let Ok(texture) = gdk::Texture::from_bytes(&bytes) {
                        self.laptop_image.set_paintable(Some(&texture));
                    }
                }

                self.board_row.set_subtitle(&board_name);
                self.bios_row
                    .set_subtitle(&format!("{bios_version} / {bios_date}"));
                self.kernel_row.set_subtitle(&kernel);
            }
            HomeCommandOutput::SerialRevealed(Ok(serial)) => {
                self.serial_row.set_subtitle(&serial);
            }
            HomeCommandOutput::SerialRevealed(Err(e)) => {
                self.reveal_button.set_sensitive(true);
                let _ = sender.output(HomeOutput::Error(e));
            }
            HomeCommandOutput::MetricsRefreshed {
                battery,
                cpu,
                ram,
                disk,
            } => {
                self.battery_label.set_label(&battery);
                self.cpu_label.set_label(&cpu);
                self.ram_label.set_label(&ram);
                self.disk_label.set_label(&disk);
            }
        }
    }
}

impl HomeModel {
    fn rebuild_profile_cards(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.profiles_flow.first_child() {
            self.profiles_flow.remove(&child);
        }
        for profile in &self.profiles {
            let card = build_profile_card(profile, &self.active_profile_id, sender);
            self.profiles_flow.append(&card);
        }
        let add_card = build_add_profile_card(sender);
        self.profiles_flow.append(&add_card);
    }
}
