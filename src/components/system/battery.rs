use gtk4::glib;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::services::commands::pkexec_shell;
use crate::services::config::AppConfig;
use crate::services::dbus;

pub struct BatteryModel {
    asusd_verfuegbar: bool,
    wartungsmodus_aktiv: bool,
    volle_aufladung_aktiv: bool,
    tiefschlaf_aktiv: bool,
    timer_abbrechen: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Debug)]
pub enum BatteryMsg {
    WartungsmodusUmschalten(bool),
    VolleAufladungUmschalten(bool),
    TiefschlafhilfeUmschalten(bool),
}

#[derive(Debug)]
pub enum BatteryCommandOutput {
    AsusdGeprueft(bool),
    LadelimitGesetzt(u8),
    Fehler(String),
    TimerAbgelaufen,
    InitWert(u8),
    InitTiefschlaf(bool),
    TiefschlafGesetzt(bool),
}

#[relm4::component(pub)]
impl Component for BatteryModel {
    type Init = ();
    type Input = BatteryMsg;
    type Output = String;
    type CommandOutput = BatteryCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &glib::markup_escape_text(&t!("battery_group_title")),
            set_description: Some(&t!("battery_group_desc")),

            add = &gtk::Label {
                #[watch]
                set_visible: !model.asusd_verfuegbar,
                set_label: &t!("asusd_missing_warning"),
                add_css_class: "error",
                set_wrap: true,
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
            },

            add = &adw::SwitchRow {
                set_title: &t!("battery_maintenance_title"),
                set_subtitle: &t!("battery_maintenance_subtitle"),

                #[watch]
                set_active: model.wartungsmodus_aktiv,

                #[watch]
                set_sensitive: model.asusd_verfuegbar,

                connect_active_notify[sender] => move |switch| {
                    sender.input(BatteryMsg::WartungsmodusUmschalten(switch.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("battery_full_charge_title"),
                set_subtitle: &t!("battery_full_charge_subtitle"),

                #[watch]
                set_active: model.volle_aufladung_aktiv,

                #[watch]
                set_sensitive: model.asusd_verfuegbar && model.wartungsmodus_aktiv,

                connect_active_notify[sender] => move |switch| {
                    sender.input(BatteryMsg::VolleAufladungUmschalten(switch.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("battery_deep_sleep_title"),
                set_subtitle: &t!("battery_deep_sleep_subtitle"),

                #[watch]
                set_active: model.tiefschlaf_aktiv,

                connect_active_notify[sender] => move |switch| {
                    sender.input(BatteryMsg::TiefschlafhilfeUmschalten(switch.is_active()));
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = BatteryModel {
            asusd_verfuegbar: false,
            wartungsmodus_aktiv: false,
            volle_aufladung_aktiv: false,
            tiefschlaf_aktiv: false,
            timer_abbrechen: None,
        };
        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    let verfuegbar = dbus::check_asusd_available().await;
                    out.emit(BatteryCommandOutput::AsusdGeprueft(verfuegbar));
                })
                .drop_on_shutdown()
        });

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    match dbus::get_charge_limit().await {
                        Ok(val) => out.emit(BatteryCommandOutput::InitWert(val)),
                        Err(e) => out.emit(BatteryCommandOutput::Fehler(e)),
                    }
                })
                .drop_on_shutdown()
        });

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    match tokio::fs::read_to_string("/sys/power/mem_sleep").await {
                        Ok(content) => {
                            let aktiv = content.contains("[deep]");
                            out.emit(BatteryCommandOutput::InitTiefschlaf(aktiv));
                        }
                        Err(e) => {
                            out.emit(BatteryCommandOutput::Fehler(
                                t!("error_mem_sleep_read", error = e.to_string()).to_string(),
                            ));
                        }
                    }
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: BatteryMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            BatteryMsg::WartungsmodusUmschalten(aktiv) => {
                if aktiv == self.wartungsmodus_aktiv {
                    return;
                }
                self.wartungsmodus_aktiv = aktiv;

                if !aktiv {
                    self.volle_aufladung_aktiv = false;
                    if let Some(cancel) = self.timer_abbrechen.take() {
                        let _ = cancel.send(());
                    }
                    sender.command(|out, shutdown| {
                        shutdown
                            .register(async move {
                                emit_limit_result(&out, 100).await;
                            })
                            .drop_on_shutdown()
                    });
                } else {
                    sender.command(|out, shutdown| {
                        shutdown
                            .register(async move {
                                emit_limit_result(&out, 80).await;
                            })
                            .drop_on_shutdown()
                    });
                }
            }
            BatteryMsg::TiefschlafhilfeUmschalten(aktiv) => {
                if aktiv == self.tiefschlaf_aktiv {
                    return;
                }
                self.tiefschlaf_aktiv = aktiv;
                AppConfig::update(|c| c.battery_tiefschlaf_aktiv = aktiv);
                let wert = if aktiv { "deep" } else { "s2idle" };
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            let cmd = format!("echo {wert} > /sys/power/mem_sleep");
                            match pkexec_shell(&cmd).await {
                                Ok(()) => out.emit(BatteryCommandOutput::TiefschlafGesetzt(aktiv)),
                                Err(e) => out.emit(BatteryCommandOutput::Fehler(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            BatteryMsg::VolleAufladungUmschalten(aktiv) => {
                if aktiv == self.volle_aufladung_aktiv {
                    return;
                }
                self.volle_aufladung_aktiv = aktiv;

                if let Some(cancel) = self.timer_abbrechen.take() {
                    let _ = cancel.send(());
                }

                if aktiv {
                    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
                    self.timer_abbrechen = Some(tx);

                    sender.command(|out, shutdown| {
                        shutdown
                            .register(async move {
                                emit_limit_result(&out, 100).await;

                                tokio::select! {
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)) => {
                                        out.emit(BatteryCommandOutput::TimerAbgelaufen);
                                    }
                                    _ = rx => {}
                                }
                            })
                            .drop_on_shutdown()
                    });
                } else {
                    sender.command(|out, shutdown| {
                        shutdown
                            .register(async move {
                                emit_limit_result(&out, 80).await;
                            })
                            .drop_on_shutdown()
                    });
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: BatteryCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            BatteryCommandOutput::AsusdGeprueft(verfuegbar) => {
                self.asusd_verfuegbar = verfuegbar;
            }
            BatteryCommandOutput::InitWert(val) => {
                self.wartungsmodus_aktiv = val <= 80;
                self.volle_aufladung_aktiv = false;
            }
            BatteryCommandOutput::InitTiefschlaf(aktiv) => {
                self.tiefschlaf_aktiv = aktiv;
            }
            BatteryCommandOutput::TiefschlafGesetzt(aktiv) => {
                let value = if aktiv { "deep" } else { "s2idle" };
                eprintln!("{}", t!("battery_deep_sleep_set", value = value));
            }
            BatteryCommandOutput::LadelimitGesetzt(val) => {
                eprintln!(
                    "{}",
                    t!("battery_charge_limit_set", value = val.to_string())
                );
            }
            BatteryCommandOutput::Fehler(e) => {
                let _ = sender.output(e);
            }
            BatteryCommandOutput::TimerAbgelaufen => {
                self.volle_aufladung_aktiv = false;
                self.timer_abbrechen = None;
                sender.command(|out, shutdown| {
                    shutdown
                        .register(async move {
                            emit_limit_result(&out, 80).await;
                        })
                        .drop_on_shutdown()
                });
            }
        }
    }
}

async fn emit_limit_result(out: &relm4::Sender<BatteryCommandOutput>, value: u8) {
    match dbus::set_charge_limit(value).await {
        Ok(val) => out.emit(BatteryCommandOutput::LadelimitGesetzt(val)),
        Err(e) => out.emit(BatteryCommandOutput::Fehler(e)),
    }
}
