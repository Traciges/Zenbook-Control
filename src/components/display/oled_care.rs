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

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use super::helpers::run_qdbus;
use crate::services::commands::{is_kde_desktop, run_command_blocking};
use crate::services::config::AppConfig;

pub struct OledCareModel {
    pixel_refresh_active: bool,
    panel_autohide_active: bool,
    transparency_active: bool,
    kde_available: bool,
}

#[derive(Debug)]
pub enum OledCareMsg {
    TogglePixelRefresh(bool),
    TogglePanelAutohide(bool),
    ToggleTransparency(bool),
    LoadProfile {
        pixel_refresh: bool,
        panel_autohide: bool,
        transparency: bool,
    },
}

#[derive(Debug)]
pub enum OledCareCommandOutput {
    PanelSet(bool),
    TransparencySet(bool),
    PixelRefreshSet(bool),
    Error(String),
}

#[relm4::component(pub)]
impl Component for OledCareModel {
    type Init = ();
    type Input = OledCareMsg;
    type Output = String;
    type CommandOutput = OledCareCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("oled_care_group_title"),
            set_description: Some(&t!("oled_care_group_desc")),

            #[template]
            add = &crate::components::widgets::DaemonWarningLabel {
                #[watch]
                set_visible: !model.kde_available,
                set_label: &t!("kde_required_warning"),
            },

            #[template]
            add = &crate::components::widgets::DaemonWarningLabel {
                set_label: &t!("oled_care_group_notice"),
            },

            add = &adw::SwitchRow {
                set_title: &t!("oled_care_pixel_refresh_title"),
                set_subtitle: &t!("oled_care_pixel_refresh_subtitle"),

                #[watch]
                set_active: model.pixel_refresh_active,
                #[watch]
                set_sensitive: model.kde_available,

                connect_active_notify[sender] => move |switch| {
                    sender.input(OledCareMsg::TogglePixelRefresh(switch.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("oled_care_panel_autohide_title"),
                set_subtitle: &t!("oled_care_panel_autohide_subtitle"),

                #[watch]
                set_active: model.panel_autohide_active,
                #[watch]
                set_sensitive: model.kde_available,

                connect_active_notify[sender] => move |switch| {
                    sender.input(OledCareMsg::TogglePanelAutohide(switch.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("oled_care_transparency_title"),
                set_subtitle: &t!("oled_care_transparency_subtitle"),

                #[watch]
                set_active: model.transparency_active,
                #[watch]
                set_sensitive: model.kde_available,

                connect_active_notify[sender] => move |switch| {
                    sender.input(OledCareMsg::ToggleTransparency(switch.is_active()));
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let config = AppConfig::load();
        let p = config.active_profile();
        let model = OledCareModel {
            pixel_refresh_active: p.oled_care_pixel_refresh,
            panel_autohide_active: p.oled_care_panel_autohide,
            transparency_active: p.oled_care_transparency,
            kde_available: is_kde_desktop(),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: OledCareMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            OledCareMsg::TogglePixelRefresh(active) => {
                if active == self.pixel_refresh_active {
                    return;
                }
                self.pixel_refresh_active = active;

                AppConfig::update(|c| c.active_profile_mut().oled_care_pixel_refresh = active);

                let idle_time = if active { "300" } else { "600" };
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match run_command_blocking(
                                "kwriteconfig6",
                                &[
                                    "--file",
                                    "powermanagementprofilesrc",
                                    "--group",
                                    "AC",
                                    "--group",
                                    "DPMSControl",
                                    "--key",
                                    "idleTime",
                                    idle_time,
                                ],
                            )
                            .await
                            {
                                Ok(()) => out.emit(OledCareCommandOutput::PixelRefreshSet(active)),
                                Err(e) => out.emit(OledCareCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            OledCareMsg::TogglePanelAutohide(active) => {
                if active == self.panel_autohide_active {
                    return;
                }
                self.panel_autohide_active = active;

                AppConfig::update(|c| c.active_profile_mut().oled_care_panel_autohide = active);

                let hiding = if active { "autohide" } else { "none" };
                let script = format!("panels().forEach(function(p){{p.hiding='{}';}})", hiding);
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            plasmashell_evaluate(
                                &script,
                                &out,
                                OledCareCommandOutput::PanelSet(active),
                            )
                            .await;
                        })
                        .drop_on_shutdown()
                });
            }
            OledCareMsg::ToggleTransparency(active) => {
                if active == self.transparency_active {
                    return;
                }
                self.transparency_active = active;

                AppConfig::update(|c| c.active_profile_mut().oled_care_transparency = active);

                let opacity = if active { "transparent" } else { "opaque" };
                let script = format!("panels().forEach(function(p){{p.opacity='{}';}})", opacity);
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            plasmashell_evaluate(
                                &script,
                                &out,
                                OledCareCommandOutput::TransparencySet(active),
                            )
                            .await;
                        })
                        .drop_on_shutdown()
                });
            }
            OledCareMsg::LoadProfile {
                pixel_refresh,
                panel_autohide,
                transparency,
            } => {
                if !self.kde_available {
                    return;
                }
                sender.input(OledCareMsg::TogglePixelRefresh(pixel_refresh));
                sender.input(OledCareMsg::TogglePanelAutohide(panel_autohide));
                sender.input(OledCareMsg::ToggleTransparency(transparency));
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: OledCareCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            OledCareCommandOutput::PanelSet(active) => {
                let value = if active { "autohide" } else { "none" };
                tracing::info!("{}", t!("oled_care_panel_set", value = value));
            }
            OledCareCommandOutput::TransparencySet(active) => {
                let value = if active { "transparent" } else { "opaque" };
                tracing::info!("{}", t!("oled_care_transparency_set", value = value));
            }
            OledCareCommandOutput::PixelRefreshSet(active) => {
                let value = if active { "300s" } else { "600s" };
                tracing::info!("{}", t!("oled_care_dpms_set", value = value));
            }
            OledCareCommandOutput::Error(e) => {
                let _ = sender.output(e);
            }
        }
    }
}

async fn plasmashell_evaluate(
    script: &str,
    out: &relm4::Sender<OledCareCommandOutput>,
    success_output: OledCareCommandOutput,
) {
    let args = vec![
        "org.kde.plasmashell".to_string(),
        "/PlasmaShell".to_string(),
        "org.kde.PlasmaShell.evaluateScript".to_string(),
        script.to_string(),
    ];
    match run_qdbus(args).await {
        Ok(()) => out.emit(success_output),
        Err(e) => out.emit(OledCareCommandOutput::Error(e)),
    }
}
