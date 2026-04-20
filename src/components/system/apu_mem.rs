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
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::services::config::AppConfig;
use crate::services::dbus;

/// Returns the display label for an APU memory value.
///
/// `0` maps to the localised "Auto" string; any positive value maps to `"N GB"`.
fn label_for_value(v: i32) -> String {
    if v == 0 {
        t!("apu_mem_value_auto").to_string()
    } else {
        t!("apu_mem_value_gb", size = v).to_string()
    }
}

/// State for the APU memory (UMA frame buffer) settings component.
pub struct ApuMemModel {
    /// `true` once the daemon confirms this attribute is available; controls sensitivity.
    available: bool,
    /// The currently applied value, used to suppress no-op callback invocations.
    current_value: i32,
    /// The allowed values returned by `possible_values`, used to resolve dropdown indices.
    display_options: Vec<i32>,
    /// Stored reference so `update_cmd` can imperatively update the model and selection.
    combo_row: adw::ComboRow,
}

/// Input messages for the APU memory component.
#[derive(Debug)]
pub enum ApuMemMsg {
    /// User selected index `idx` in the memory size dropdown.
    ChangeValue(u32),
    /// Apply APU memory value from a profile without saving.
    LoadProfile(i32),
}

/// Async command results for the APU memory component.
#[derive(Debug)]
pub enum ApuMemCommandOutput {
    /// asusd is offline or the BIOS does not expose the `apu_mem` attribute.
    NotAvailable,
    /// Initial options and current value successfully read from the daemon.
    Init(Vec<i32>, i32),
    /// Confirmation that `SetCurrentValue` succeeded; carries the applied value.
    ValueSet(i32),
    /// An error message to forward as a toast notification.
    Error(String),
}

#[relm4::component(pub)]
impl Component for ApuMemModel {
    type Init = ();
    type Input = ApuMemMsg;
    type Output = String;
    type CommandOutput = ApuMemCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("apu_mem_group_title"),
            set_description: Some(&t!("apu_mem_group_desc")),

            add = &gtk::Label {
                #[watch]
                set_visible: !model.available,
                set_label: &t!("apu_mem_not_supported_warning"),
                add_css_class: "error",
                set_wrap: true,
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
            },

            add = &model.combo_row.clone() -> adw::ComboRow {
                set_title: &t!("apu_mem_title"),
                set_subtitle: &t!("apu_mem_subtitle"),

                #[watch]
                set_sensitive: model.available,
            },

            add = &gtk::Label {
                set_label: &t!("apu_mem_reboot_warning"),
                add_css_class: "dim-label",
                set_wrap: true,
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let combo_row = adw::ComboRow::new();

        combo_row.connect_selected_notify({
            let sender = sender.clone();
            move |row| {
                sender.input(ApuMemMsg::ChangeValue(row.selected()));
            }
        });

        let saved_value = AppConfig::load().active_profile().apu_mem;

        // Set a placeholder model so the ComboRow always renders,
        // even before the daemon responds or when it is unavailable.
        combo_row.set_model(Some(&gtk::StringList::new(&[&label_for_value(saved_value)])));

        let model = ApuMemModel {
            available: false,
            current_value: saved_value,
            display_options: Vec::new(),
            combo_row,
        };

        let widgets = view_output!();

        // Single command: try to read options. If this fails for any reason
        // (asusd offline, BIOS doesn't expose apu_mem), leave the group disabled.
        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    let options = match dbus::get_apu_mem_options().await {
                        Ok(v) if !v.is_empty() => v,
                        _ => {
                            out.emit(ApuMemCommandOutput::NotAvailable);
                            return;
                        }
                    };
                    let current = match dbus::get_apu_mem().await {
                        Ok(v) => v,
                        Err(_) => {
                            out.emit(ApuMemCommandOutput::NotAvailable);
                            return;
                        }
                    };
                    out.emit(ApuMemCommandOutput::Init(options, current));
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ApuMemMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            ApuMemMsg::LoadProfile(value) => {
                if !self.available {
                    return;
                }
                self.current_value = value;
                if let Some(idx) = self.display_options.iter().position(|&v| v == value) {
                    self.combo_row.set_selected(idx as u32);
                }
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus::set_apu_mem(value).await {
                                Ok(v) => out.emit(ApuMemCommandOutput::ValueSet(v)),
                                Err(e) => out.emit(ApuMemCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            ApuMemMsg::ChangeValue(idx) => {
                let Some(&value) = self.display_options.get(idx as usize) else {
                    return;
                };
                if value == self.current_value {
                    return;
                }
                self.current_value = value;
                AppConfig::update(|c| c.active_profile_mut().apu_mem = value);

                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus::set_apu_mem(value).await {
                                Ok(v) => out.emit(ApuMemCommandOutput::ValueSet(v)),
                                Err(e) => out.emit(ApuMemCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: ApuMemCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            ApuMemCommandOutput::NotAvailable => {
                // available stays false; group is visible but combo row is disabled
            }
            ApuMemCommandOutput::Init(options, current) => {
                self.available = true;
                self.display_options = options;
                self.current_value = current;

                let translated: Vec<String> =
                    self.display_options.iter().map(|&v| label_for_value(v)).collect();
                let str_refs: Vec<&str> = translated.iter().map(|s| s.as_str()).collect();
                self.combo_row.set_model(Some(&gtk::StringList::new(&str_refs)));

                let idx = self
                    .display_options
                    .iter()
                    .position(|&v| v == current)
                    .unwrap_or(0) as u32;
                self.combo_row.set_selected(idx);
            }
            ApuMemCommandOutput::ValueSet(v) => {
                tracing::info!("{}", t!("apu_mem_set", value = label_for_value(v)));
            }
            ApuMemCommandOutput::Error(e) => {
                let _ = sender.output(e);
            }
        }
    }
}
