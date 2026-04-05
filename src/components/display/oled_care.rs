use gtk4 as gtk;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use super::helpers::run_qdbus;
use crate::services::commands::run_command_blocking;
use crate::services::config::AppConfig;

pub struct OledCareModel {
    pixel_refresh_active: bool,
    panel_autohide_active: bool,
    transparency_active: bool,
}

#[derive(Debug)]
pub enum OledCareMsg {
    TogglePixelRefresh(bool),
    TogglePanelAutohide(bool),
    ToggleTransparency(bool),
}

#[derive(Debug)]
pub enum OledCareCommandOutput {
    PanelSet(bool),
    TransparencySet(bool),
    PixelRefreshSet(bool),
    Fehler(String),
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

            add = &gtk::Label {
                set_label: &t!("oled_care_group_notice"),
                add_css_class: "error",
                set_wrap: true,
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
            },

            add = &adw::SwitchRow {
                set_title: &t!("oled_care_pixel_refresh_title"),
                set_subtitle: &t!("oled_care_pixel_refresh_subtitle"),

                #[watch]
                set_active: model.pixel_refresh_active,

                connect_active_notify[sender] => move |switch| {
                    sender.input(OledCareMsg::TogglePixelRefresh(switch.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("oled_care_panel_autohide_title"),
                set_subtitle: &t!("oled_care_panel_autohide_subtitle"),

                #[watch]
                set_active: model.panel_autohide_active,

                connect_active_notify[sender] => move |switch| {
                    sender.input(OledCareMsg::TogglePanelAutohide(switch.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("oled_care_transparency_title"),
                set_subtitle: &t!("oled_care_transparency_subtitle"),

                #[watch]
                set_active: model.transparency_active,

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
        let model = OledCareModel {
            pixel_refresh_active: config.oled_care_pixel_refresh,
            panel_autohide_active: config.oled_care_panel_autohide,
            transparency_active: config.oled_care_transparenz,
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

                AppConfig::update(|c| c.oled_care_pixel_refresh = active);

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
                                Ok(()) => {
                                    out.emit(OledCareCommandOutput::PixelRefreshSet(active))
                                }
                                Err(e) => out.emit(OledCareCommandOutput::Fehler(e)),
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

                AppConfig::update(|c| c.oled_care_panel_autohide = active);

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

                AppConfig::update(|c| c.oled_care_transparenz = active);

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
            OledCareCommandOutput::Fehler(e) => {
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
        Err(e) => out.emit(OledCareCommandOutput::Fehler(e)),
    }
}
