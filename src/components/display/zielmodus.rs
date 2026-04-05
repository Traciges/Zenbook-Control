use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use super::helpers::run_qdbus;
use crate::services::commands::{is_kde_desktop, run_command_blocking};
use crate::services::config::AppConfig;

pub struct ZielmodusModel {
    active: bool,
    kde_available: bool,
}

#[derive(Debug)]
pub enum ZielmodusMsg {
    SetActive(bool),
}

#[derive(Debug)]
pub enum ZielmodusCommandOutput {
    ActiveRead(bool),
    ActiveSet(bool),
    Fehler(String),
}

#[relm4::component(pub)]
impl Component for ZielmodusModel {
    type Init = ();
    type Input = ZielmodusMsg;
    type Output = String;
    type CommandOutput = ZielmodusCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("zielmodus_group_title"),
            set_description: Some(&t!("zielmodus_group_desc")),

            add = &gtk::Label {
                #[watch]
                set_visible: !model.kde_available,
                set_label: &t!("zielmodus_kde_required"),
                add_css_class: "error",
                set_wrap: true,
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
            },

            add = &adw::SwitchRow {
                set_title: &t!("zielmodus_switch_title"),
                set_subtitle: &t!("zielmodus_switch_subtitle"),

                #[watch]
                set_active: model.active,
                #[watch]
                set_sensitive: model.kde_available,

                connect_active_notify[sender] => move |switch| {
                    sender.input(ZielmodusMsg::SetActive(switch.is_active()));
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
        let kde_available = is_kde_desktop();

        let model = ZielmodusModel {
            active: config.zielmodus_aktiv,
            kde_available,
        };
        let widgets = view_output!();

        if kde_available {
            let fallback = config.zielmodus_aktiv;
            sender.command(move |out, shutdown| {
                shutdown
                    .register(async move {
                        let active = tokio::task::spawn_blocking(move || {
                            read_kwin_bool("Plugins", "diminactiveEnabled")
                        })
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or(fallback);
                        out.emit(ZielmodusCommandOutput::ActiveRead(active));
                    })
                    .drop_on_shutdown()
            });
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ZielmodusMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            ZielmodusMsg::SetActive(active) => {
                if active == self.active {
                    return;
                }
                self.active = active;
                AppConfig::update(|c| c.zielmodus_aktiv = active);

                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match set_kwin_effect(active).await {
                                Ok(()) => out.emit(ZielmodusCommandOutput::ActiveSet(active)),
                                Err(e) => out.emit(ZielmodusCommandOutput::Fehler(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: ZielmodusCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            ZielmodusCommandOutput::ActiveRead(active) => {
                self.active = active;
                AppConfig::update(|c| c.zielmodus_aktiv = active);
            }
            ZielmodusCommandOutput::ActiveSet(active) => {
                tracing::info!("{}", t!("zielmodus_aktiv_set", value = active.to_string()));
            }
            ZielmodusCommandOutput::Fehler(e) => {
                let _ = sender.output(e);
            }
        }
    }
}

async fn set_kwin_effect(active: bool) -> Result<(), String> {
    let value = if active { "true" } else { "false" };
    run_command_blocking(
        "kwriteconfig6",
        &[
            "--file",
            "kwinrc",
            "--group",
            "Plugins",
            "--key",
            "diminactiveEnabled",
            "--type",
            "bool",
            value,
        ],
    )
    .await?;

    let method = if active { "loadEffect" } else { "unloadEffect" };
    run_qdbus(vec![
        "org.kde.KWin".to_string(),
        "/Effects".to_string(),
        method.to_string(),
        "diminactive".to_string(),
    ])
    .await
}

fn read_kwin_bool(group: &str, key: &str) -> Option<bool> {
    let output = std::process::Command::new("kreadconfig6")
        .args([
            "--file",
            "kwinrc",
            "--group",
            group,
            "--key",
            key,
            "--default",
            "false",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_lowercase();
    Some(s == "true")
}
