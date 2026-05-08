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
use gtk4::prelude::*;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::components::aura::aura_device::{AuraDeviceModel, AuraDeviceMsg};
use crate::services::dbus::{self, AuraDeviceInfo, AuraDeviceKind};

/// Top-level Aura page. Discovers every Aura device exposed by `asusd` and
/// renders one [`AuraDeviceModel`] per device. Falls back to a status label
/// when the daemon is missing or no Aura devices are reported.
pub struct AuraPageModel {
    container: gtk::Box,
    status_group: adw::PreferencesGroup,
    daemon_label: gtk::Label,
    no_devices_label: gtk::Label,
    devices: Vec<(AuraDeviceKind, Controller<AuraDeviceModel>)>,
}

#[derive(Debug)]
pub enum AuraPageMsg {
    LoadProfile {
        mode: u32,
        zone: u32,
        brightness: u32,
        colour_r: u8,
        colour_g: u8,
        colour_b: u8,
        colour2_r: u8,
        colour2_g: u8,
        colour2_b: u8,
        speed: String,
        direction: String,
    },
}

#[derive(Debug)]
pub enum AuraPageCmd {
    Discovered {
        daemon_running: bool,
        devices: Vec<AuraDeviceInfo>,
    },
    DiscoveryError(String),
}

impl Component for AuraPageModel {
    type Init = ();
    type Input = AuraPageMsg;
    type Output = String;
    type CommandOutput = AuraPageCmd;
    type Root = gtk::Box;
    type Widgets = ();

    fn init_root() -> Self::Root {
        gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .build()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let status_group = adw::PreferencesGroup::new();
        status_group.set_title(&t!("aura_group_title"));
        status_group.set_description(Some(&t!("aura_group_desc")));

        let daemon_label = gtk::Label::new(Some(&t!("aura_daemon_missing_warning")));
        daemon_label.add_css_class("error");
        daemon_label.set_wrap(true);
        daemon_label.set_xalign(0.0);
        daemon_label.set_margin_top(8);
        daemon_label.set_margin_start(12);
        daemon_label.set_margin_end(12);
        daemon_label.set_margin_bottom(4);
        daemon_label.set_visible(false);
        status_group.add(&daemon_label);

        let no_devices_label = gtk::Label::new(Some(&t!("aura_no_devices_info")));
        no_devices_label.add_css_class("dim-label");
        no_devices_label.set_wrap(true);
        no_devices_label.set_xalign(0.0);
        no_devices_label.set_margin_top(8);
        no_devices_label.set_margin_start(12);
        no_devices_label.set_margin_end(12);
        no_devices_label.set_margin_bottom(4);
        no_devices_label.set_visible(false);
        status_group.add(&no_devices_label);

        root.append(&status_group);

        let model = AuraPageModel {
            container: root.clone(),
            status_group,
            daemon_label,
            no_devices_label,
            devices: Vec::new(),
        };

        sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    let daemon_running = dbus::is_asusd_running().await;
                    if !daemon_running {
                        out.emit(AuraPageCmd::Discovered {
                            daemon_running: false,
                            devices: Vec::new(),
                        });
                        return;
                    }
                    match dbus::discover_aura_devices().await {
                        Ok(devices) => out.emit(AuraPageCmd::Discovered {
                            daemon_running: true,
                            devices,
                        }),
                        Err(e) => out.emit(AuraPageCmd::DiscoveryError(e)),
                    }
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets: () }
    }

    fn update(&mut self, msg: AuraPageMsg, _sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            AuraPageMsg::LoadProfile {
                mode,
                zone,
                brightness,
                colour_r,
                colour_g,
                colour_b,
                colour2_r,
                colour2_g,
                colour2_b,
                speed,
                direction,
            } => {
                // Apply only to the first keyboard device — profiles target the
                // primary keyboard, not lightbar / lid / anime devices.
                if let Some((_, ctrl)) = self.devices.iter().find(|(k, _)| k.is_keyboard()) {
                    ctrl.sender().emit(AuraDeviceMsg::LoadProfile {
                        mode,
                        zone,
                        brightness,
                        colour_r,
                        colour_g,
                        colour_b,
                        colour2_r,
                        colour2_g,
                        colour2_b,
                        speed,
                        direction,
                    });
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: AuraPageCmd,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AuraPageCmd::Discovered {
                daemon_running,
                devices,
            } => {
                self.daemon_label.set_visible(!daemon_running);
                self.no_devices_label
                    .set_visible(daemon_running && devices.is_empty());
                self.status_group
                    .set_visible(!daemon_running || devices.is_empty());

                for info in devices {
                    let label = format_device_subtitle(&info);
                    let kind = info.kind;
                    let ctrl = AuraDeviceModel::builder()
                        .launch(info)
                        .detach();
                    let widget = ctrl.widget();
                    widget.set_description(Some(&label));
                    self.container.append(widget);
                    self.devices.push((kind, ctrl));
                }
            }
            AuraPageCmd::DiscoveryError(e) => {
                let _ = sender.output(e);
                self.daemon_label.set_visible(false);
                self.no_devices_label.set_visible(true);
                self.status_group.set_visible(true);
            }
        }
    }
}

fn format_device_subtitle(info: &AuraDeviceInfo) -> String {
    // Last path segment is the most useful identifier in practice.
    let suffix = info.path.rsplit('/').next().unwrap_or("");
    match info.kind {
        AuraDeviceKind::Keyboard | AuraDeviceKind::KeyboardTuf => suffix.to_string(),
        _ => suffix.to_string(),
    }
}

