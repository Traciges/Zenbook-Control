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

use gtk::gdk;
use gtk::glib;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;
use tokio::sync::watch;

use crate::services::config::AppConfig;
use crate::services::edge_gestures;

pub struct GesturesModel {
    active: bool,
    loop_tx: Option<watch::Sender<bool>>,
    brightnessctl_available: bool,
    playerctl_available: bool,
}

#[derive(Debug)]
pub enum GesturesMsg {
    ToggleGestures(bool),
    LoadProfile(bool),
}

#[derive(Debug)]
pub enum GesturesCommandOutput {
    BrightnessctlChecked(bool),
    PlayerctlChecked(bool),
}

const GESTURE_IMG: &[u8] = include_bytes!("../../../assets/img/gesture.png");

#[relm4::component(pub)]
impl Component for GesturesModel {
    type Init = ();
    type Input = GesturesMsg;
    type Output = String;
    type CommandOutput = GesturesCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("gestures_group_title"),
            set_description: Some(&t!("gestures_group_desc")),

            #[template]
            add = &crate::components::widgets::DaemonWarningLabel {
                #[watch]
                set_visible: !model.brightnessctl_available || !model.playerctl_available,
                set_label: &t!("gestures_deps_missing_warning"),
            },

            add = &gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 24,

                #[name = "gesture_image"]
                append = &gtk::Picture {
                    set_width_request: 300,
                    set_valign: gtk::Align::Start,
                },

                append = &gtk::ListBox {
                    set_hexpand: true,
                    set_valign: gtk::Align::Start,
                    add_css_class: "boxed-list",

                    append = &adw::SwitchRow {
                        set_title: &t!("gestures_toggle_title"),

                        #[watch]
                        set_active: model.active,

                        #[watch]
                        set_sensitive: model.brightnessctl_available && model.playerctl_available,

                        connect_active_notify[sender] => move |s| {
                            sender.input(GesturesMsg::ToggleGestures(s.is_active()));
                        },
                    },

                    append = &adw::ActionRow {
                        set_title: &t!("gestures_volume_title"),
                        set_subtitle: &t!("gestures_volume_subtitle"),
                    },

                    append = &adw::ActionRow {
                        set_title: &t!("gestures_brightness_title"),
                        set_subtitle: &t!("gestures_brightness_subtitle"),
                    },

                    append = &adw::ActionRow {
                        set_title: &t!("gestures_media_title"),
                        set_subtitle: &t!("gestures_media_subtitle"),
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let active = AppConfig::load().active_profile().input_gestures_active;
        let loop_tx = if active {
            Some(start_gesture_loop())
        } else {
            None
        };
        let model = GesturesModel {
            active,
            loop_tx,
            brightnessctl_available: false,
            playerctl_available: false,
        };
        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    let ok = crate::services::commands::which_exists("brightnessctl").await;
                    out.send(GesturesCommandOutput::BrightnessctlChecked(ok)).ok();
                })
                .drop_on_shutdown()
        });

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    let ok = crate::services::commands::which_exists("playerctl").await;
                    out.send(GesturesCommandOutput::PlayerctlChecked(ok)).ok();
                })
                .drop_on_shutdown()
        });

        let bytes = glib::Bytes::from_static(GESTURE_IMG);
        if let Ok(texture) = gdk::Texture::from_bytes(&bytes) {
            widgets.gesture_image.set_paintable(Some(&texture));
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: GesturesMsg, _sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            GesturesMsg::ToggleGestures(active) => {
                if active == self.active {
                    return;
                }
                self.active = active;
                AppConfig::update(|c| c.active_profile_mut().input_gestures_active = active);

                if active {
                    self.loop_tx = Some(start_gesture_loop());
                } else {
                    self.loop_tx = None;
                }
            }
            GesturesMsg::LoadProfile(active) => {
                self.active = active;
                if active {
                    self.loop_tx = Some(start_gesture_loop());
                } else {
                    self.loop_tx = None;
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: GesturesCommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            GesturesCommandOutput::BrightnessctlChecked(ok) => self.brightnessctl_available = ok,
            GesturesCommandOutput::PlayerctlChecked(ok) => self.playerctl_available = ok,
        }
    }
}

fn start_gesture_loop() -> watch::Sender<bool> {
    let (tx, rx) = watch::channel(true);
    tokio::spawn(edge_gestures::run_gesture_loop(rx));
    tx
}
