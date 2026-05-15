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
use tokio::sync::{mpsc, watch};

use crate::services::config::AppConfig;
use crate::services::numberpad::{self, NumberpadStatus};

pub struct NumberpadModel {
    /// Master "feature enabled" state - mirrors `Profile.numberpad_active`.
    /// When true, the background loop is running; when false, no Tokio tasks.
    enabled: bool,
    /// Runtime "Active mode" state - LEDs on / touchpad grabbed when true.
    /// Toggled by `--toggle-numberpad` IPC or by the corner-tap gesture
    /// (future). Not persisted.
    active: bool,
    /// Result of the startup hardware probe. Drives sensitivity + warning.
    status: NumberpadStatus,
    /// Drops the running service loop. Some(_) iff the loop is alive.
    shutdown_tx: Option<watch::Sender<bool>>,
    /// Sends new Active/Idle state to the running loop. Some(_) iff alive.
    active_tx: Option<watch::Sender<bool>>,
}

#[derive(Debug)]
pub enum NumberpadMsg {
    /// Component startup probe finished.
    Probed(NumberpadStatus),
    /// User flipped the master `adw::SwitchRow`.
    Toggle(bool),
    /// Profile-switch from `AppModel` - synchronise to the new profile's value.
    LoadProfile(bool),
    /// Runtime toggle (CLI / future gesture). Flips Active/Idle.
    ToggleActive,
    /// One-way feedback from the backend loop: the on-touchpad corner-tap
    /// gesture flipped the active state, the UI needs to catch up.
    SyncActive(bool),
}

#[relm4::component(pub)]
impl Component for NumberpadModel {
    type Init = ();
    type Input = NumberpadMsg;
    type Output = String;
    type CommandOutput = NumberpadStatus;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("numberpad_group_title"),
            set_description: Some(&t!("numberpad_group_desc")),

            #[template]
            add = &crate::components::widgets::DaemonWarningLabel {
                #[watch]
                set_visible: !matches!(model.status, NumberpadStatus::Ok),
                #[watch]
                set_label: &status_message(&model.status),
            },

            add = &adw::SwitchRow {
                set_title: &t!("numberpad_enable_title"),
                set_subtitle: &t!("numberpad_enable_subtitle"),

                #[watch]
                set_active: model.enabled,

                #[watch]
                set_sensitive: matches!(model.status, NumberpadStatus::Ok),

                connect_active_notify[sender] => move |s| {
                    sender.input(NumberpadMsg::Toggle(s.is_active()));
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let enabled = AppConfig::load().active_profile().numberpad_active;
        let model = NumberpadModel {
            enabled,
            active: false,
            // Pessimistic placeholder; replaced by Probed in update_cmd.
            status: NumberpadStatus::NoHardware,
            shutdown_tx: None,
            active_tx: None,
        };
        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    out.send(numberpad::probe().await).ok();
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: NumberpadMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            NumberpadMsg::Probed(status) => {
                self.status = status;
                // If hardware probe failed but the persisted profile had this
                // enabled, leave self.enabled untouched - the switch is just
                // insensitive, so the user can't toggle it on. Profile state
                // remains consistent for when they fix permissions and relaunch.
                if !matches!(self.status, NumberpadStatus::Ok) {
                    return;
                }
                if self.enabled && self.shutdown_tx.is_none() {
                    self.spawn_service(&sender);
                }
            }
            NumberpadMsg::Toggle(active) => {
                if active == self.enabled {
                    return;
                }
                self.enabled = active;
                AppConfig::update(|c| c.active_profile_mut().numberpad_active = active);
                if !matches!(self.status, NumberpadStatus::Ok) {
                    return;
                }
                if active {
                    self.spawn_service(&sender);
                } else {
                    self.stop_service();
                }
            }
            NumberpadMsg::LoadProfile(active) => {
                self.enabled = active;
                if !matches!(self.status, NumberpadStatus::Ok) {
                    return;
                }
                if active && self.shutdown_tx.is_none() {
                    self.spawn_service(&sender);
                } else if !active && self.shutdown_tx.is_some() {
                    self.stop_service();
                }
            }
            NumberpadMsg::ToggleActive => {
                if self.shutdown_tx.is_none() {
                    // Feature is off - nothing to toggle.
                    return;
                }
                self.active = !self.active;
                if let Some(tx) = &self.active_tx {
                    let _ = tx.send(self.active);
                }
            }
            NumberpadMsg::SyncActive(b) => {
                // Gesture-originated toggle: the run_loop has *already* applied
                // the new state internally. We only update local state here;
                // pushing back on `active_tx` would be a redundant no-op (the
                // value already matches) and conceptually wrong - the loop is
                // already the source of truth for this flip.
                self.active = b;
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: NumberpadStatus,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        self.update(NumberpadMsg::Probed(msg), sender, root);
    }
}

impl NumberpadModel {
    fn spawn_service(&mut self, sender: &ComponentSender<Self>) {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (active_tx, active_rx) = watch::channel(self.active);
        let (feedback_tx, mut feedback_rx) = mpsc::unbounded_channel();
        tokio::spawn(numberpad::run_loop(shutdown_rx, active_rx, feedback_tx));

        // Relay gesture-originated toggles back into the component's message
        // loop. Terminates automatically when `feedback_tx` is dropped at the
        // end of `run_loop`, which happens on shutdown.
        let relay_sender = sender.clone();
        tokio::spawn(async move {
            while let Some(b) = feedback_rx.recv().await {
                relay_sender.input(NumberpadMsg::SyncActive(b));
            }
        });

        self.shutdown_tx = Some(shutdown_tx);
        self.active_tx = Some(active_tx);
    }

    fn stop_service(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
        }
        self.active_tx = None;
        self.active = false;
    }
}

fn status_message(status: &NumberpadStatus) -> String {
    match status {
        NumberpadStatus::Ok => String::new(),
        NumberpadStatus::NoHardware => t!("numberpad_status_no_hardware").to_string(),
        NumberpadStatus::I2cUnavailable(dev) => {
            t!("numberpad_status_i2c_unavailable", device = dev).to_string()
        }
        NumberpadStatus::PermissionDenied { device } => {
            t!("numberpad_status_permission_denied", device = device).to_string()
        }
    }
}
