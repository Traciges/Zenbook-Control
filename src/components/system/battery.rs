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

use gtk4::glib;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::services::commands::pkexec_write_sysfs;
use crate::sys_paths::SYS_MEM_SLEEP;
use crate::services::config::AppConfig;
use crate::services::dbus;

/// State for the battery settings component.
pub struct BatteryModel {
    /// Whether the `asusd` daemon is reachable; disables charge controls when `false`.
    asusd_available: bool,
    /// When `true`, the charge limit is capped at 80% to extend long-term battery health.
    maintenance_mode_active: bool,
    /// When `true`, the charge limit is temporarily raised to 100% for up to 24 hours.
    full_charge_active: bool,
    /// When `true`, suspend-to-RAM uses the `deep` sleep state instead of `s2idle`.
    deep_sleep_active: bool,
    /// When `true` the `deep_sleep` is supported by device.
    deep_sleep_supported: bool,
    /// Cancels the 24-hour full-charge revert timer when the user turns off full-charge mode early.
    timer_cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

/// Input messages for the battery component.
#[derive(Debug)]
pub enum BatteryMsg {
    /// Enable or disable the 80% charge maintenance mode.
    ToggleMaintenanceMode(bool),
    /// Enable or disable the temporary 100% full-charge mode (auto-reverts after 24 h).
    ToggleFullCharge(bool),
    /// Switch suspend-to-RAM between `s2idle` (`false`) and `deep` (`true`).
    ToggleDeepSleep(bool),
    /// Apply deep-sleep setting from a profile without saving.
    LoadProfile(bool),
}

/// Async command results for the battery component.
#[derive(Debug)]
pub enum BatteryCommandOutput {
    /// Result of the initial `asusd` availability check.
    AsusdChecked(bool),
    /// Confirmation that the charge limit was successfully written.
    ChargeLimitSet(u8),
    /// An error message to forward as a toast notification.
    Error(String),
    /// Fired after the 24-hour full-charge timer expires, triggering a revert to 80%.
    TimerElapsed,
    /// Charge limit value read from `asusd` during initialisation.
    InitValue(u8),
    /// Deep-sleep state read from `/sys/power/mem_sleep` during initialisation.
    InitDeepSleep(bool),
    /// Confirmation that the `/sys/power/mem_sleep` write succeeded.
    DeepSleepSet(bool),
    /// Whether deep_sleep is supported by the device.
    DeepSleepSupported(bool),
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
                set_visible: !model.asusd_available,
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
                set_active: model.maintenance_mode_active,

                #[watch]
                set_sensitive: model.asusd_available,

                connect_active_notify[sender] => move |switch| {
                    sender.input(BatteryMsg::ToggleMaintenanceMode(switch.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("battery_full_charge_title"),
                set_subtitle: &t!("battery_full_charge_subtitle"),

                #[watch]
                set_active: model.full_charge_active,

                #[watch]
                set_sensitive: model.asusd_available && model.maintenance_mode_active,

                connect_active_notify[sender] => move |switch| {
                    sender.input(BatteryMsg::ToggleFullCharge(switch.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("battery_deep_sleep_title"),
                #[watch]
                set_subtitle: &if model.deep_sleep_supported {
                    t!("battery_deep_sleep_subtitle")
                } else {
                    t!("battery_deep_sleep_not_supported")
                },
                #[watch]
                set_sensitive: model.deep_sleep_supported,
                #[watch]
                set_active: model.deep_sleep_active,

                connect_active_notify[sender] => move |switch| {
                    sender.input(BatteryMsg::ToggleDeepSleep(switch.is_active()));
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
            asusd_available: false,
            maintenance_mode_active: false,
            full_charge_active: false,
            deep_sleep_active: false,
            deep_sleep_supported: false,
            timer_cancel: None,
        };
        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    let available = dbus::check_asusd_available().await;
                    out.emit(BatteryCommandOutput::AsusdChecked(available));

                    if !available {
                        return;
                    }

                    match dbus::get_charge_limit().await {
                        Ok(val) => out.emit(BatteryCommandOutput::InitValue(val)),
                        Err(e) => out.emit(BatteryCommandOutput::Error(e)),
                    }
                })
                .drop_on_shutdown()
        });

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    match tokio::fs::read_to_string(SYS_MEM_SLEEP).await {
                        Ok(content) => {
                            let active = content.contains("[deep]");
                            let supported = content.contains("deep");

                            out.emit(BatteryCommandOutput::InitDeepSleep(active));
                            out.emit(BatteryCommandOutput::DeepSleepSupported(supported));
                        }
                        Err(e) => {
                            out.emit(BatteryCommandOutput::Error(
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
            BatteryMsg::ToggleMaintenanceMode(active) => {
                if active == self.maintenance_mode_active {
                    return;
                }
                self.maintenance_mode_active = active;

                if !active {
                    self.full_charge_active = false;
                    if let Some(cancel) = self.timer_cancel.take() {
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
            BatteryMsg::ToggleDeepSleep(active) => {
                if active && !self.deep_sleep_supported {
                    let _ = sender.output(t!("battery_deep_sleep_not_supported").to_string());
                    return;
                }
                if active == self.deep_sleep_active {
                    return;
                }

                self.deep_sleep_active = active;
                AppConfig::update(|c| c.active_profile_mut().battery_deep_sleep_active = active);

                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            let value = if active { "deep" } else { "s2idle" };
                            match pkexec_write_sysfs(SYS_MEM_SLEEP, value).await {
                                Ok(()) => out.emit(BatteryCommandOutput::DeepSleepSet(active)),
                                Err(e) => out.emit(BatteryCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            BatteryMsg::LoadProfile(active) => {
                if self.deep_sleep_active == active || !self.deep_sleep_supported {
                    return;
                }
                self.deep_sleep_active = active;
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            let value = if active { "deep" } else { "s2idle" };
                            match pkexec_write_sysfs(SYS_MEM_SLEEP, value).await {
                                Ok(()) => out.emit(BatteryCommandOutput::DeepSleepSet(active)),
                                Err(e) => out.emit(BatteryCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            BatteryMsg::ToggleFullCharge(active) => {
                if active == self.full_charge_active {
                    return;
                }
                self.full_charge_active = active;

                if let Some(cancel) = self.timer_cancel.take() {
                    let _ = cancel.send(());
                }

                if active {
                    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
                    self.timer_cancel = Some(tx);

                    sender.command(|out, shutdown| {
                        shutdown
                            .register(async move {
                                emit_limit_result(&out, 100).await;

                                tokio::select! {
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)) => {
                                        out.emit(BatteryCommandOutput::TimerElapsed);
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
            BatteryCommandOutput::AsusdChecked(available) => {
                self.asusd_available = available;
            }
            BatteryCommandOutput::InitValue(val) => {
                self.maintenance_mode_active = val != 100;
                self.full_charge_active = false;
            }
            BatteryCommandOutput::InitDeepSleep(active) => {
                self.deep_sleep_active = active;
            }
            BatteryCommandOutput::DeepSleepSupported(supported) => {
                self.deep_sleep_supported = supported;

                if !supported {
                    self.deep_sleep_active = false;
                }
            }
            BatteryCommandOutput::DeepSleepSet(active) => {
                let value = if active && self.deep_sleep_supported {
                    "deep"
                } else {
                    "s2idle"
                };
                tracing::info!("{}", t!("battery_deep_sleep_set", value = value));
            }
            BatteryCommandOutput::ChargeLimitSet(val) => {
                tracing::info!(
                    "{}",
                    t!("battery_charge_limit_set", value = val.to_string())
                );
            }
            BatteryCommandOutput::Error(e) => {
                let _ = sender.output(e);
            }
            BatteryCommandOutput::TimerElapsed => {
                self.full_charge_active = false;
                self.timer_cancel = None;
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

/// Calls [`dbus::set_charge_limit`] and emits either [`BatteryCommandOutput::ChargeLimitSet`]
/// or [`BatteryCommandOutput::Error`] depending on the outcome.
async fn emit_limit_result(out: &relm4::Sender<BatteryCommandOutput>, value: u8) {
    match dbus::set_charge_limit(value).await {
        Ok(val) => out.emit(BatteryCommandOutput::ChargeLimitSet(val)),
        Err(e) => out.emit(BatteryCommandOutput::Error(e)),
    }
}
