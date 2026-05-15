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

use crate::services::commands::run_command_blocking;
use crate::services::config::AppConfig;

pub struct FnKeyModel {
    locked: bool,
    grubby_available: bool,
    check_locked: gtk::CheckButton,
    check_normal: gtk::CheckButton,
    row_hint: adw::ActionRow,
    row_locked: adw::ActionRow,
    row_normal: adw::ActionRow,
}

#[derive(Debug)]
pub enum FnKeyMsg {
    ToggleLocked(bool),
    LoadProfile(bool),
}

#[derive(Debug)]
pub enum FnKeyCommandOutput {
    GrubbyChecked(bool),
    Set(bool),
    Error(String),
}

#[relm4::component(pub)]
impl Component for FnKeyModel {
    type Init = ();
    type Input = FnKeyMsg;
    type Output = String;
    type CommandOutput = FnKeyCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("fn_key_group_title"),
            set_description: Some(&t!("fn_key_group_desc")),

            add = &gtk::Label {
                #[watch]
                set_visible: !model.grubby_available,
                set_label: &t!("fn_key_grubby_missing_warning"),
                add_css_class: "error",
                set_wrap: true,
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
            },

            add = &model.row_hint.clone(),
            add = &model.row_locked.clone(),
            add = &model.row_normal.clone(),
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let check_locked = gtk::CheckButton::new();
        let check_normal = gtk::CheckButton::new();

        check_normal.set_group(Some(&check_locked));

        let locked = AppConfig::load().active_profile().input_fn_key_locked;
        if locked {
            check_locked.set_active(true);
        } else {
            check_normal.set_active(true);
        }

        {
            let sender = sender.clone();
            check_locked.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(FnKeyMsg::ToggleLocked(true));
                }
            });
        }
        {
            let sender = sender.clone();
            check_normal.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(FnKeyMsg::ToggleLocked(false));
                }
            });
        }

        let row_hint = adw::ActionRow::new();
        row_hint.set_title(&t!("fn_key_hint_title"));
        row_hint.set_subtitle(&t!("fn_key_hint_subtitle"));
        row_hint.set_selectable(false);

        let row_locked = adw::ActionRow::new();
        row_locked.set_title(&t!("fn_key_locked_title"));
        row_locked.set_subtitle(&t!("fn_key_locked_subtitle"));
        row_locked.add_prefix(&check_locked);
        row_locked.set_activatable_widget(Some(&check_locked));

        let row_normal = adw::ActionRow::new();
        row_normal.set_title(&t!("fn_key_normal_title"));
        row_normal.set_subtitle(&t!("fn_key_normal_subtitle"));
        row_normal.add_prefix(&check_normal);
        row_normal.set_activatable_widget(Some(&check_normal));

        let model = FnKeyModel {
            locked,
            grubby_available: false,
            check_locked,
            check_normal,
            row_hint,
            row_locked,
            row_normal,
        };

        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    let ok = tokio::task::spawn_blocking(|| {
                        std::process::Command::new("which")
                            .arg("grubby")
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false)
                    })
                    .await
                    .unwrap_or(false);
                    out.send(FnKeyCommandOutput::GrubbyChecked(ok)).ok();
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: FnKeyMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            FnKeyMsg::LoadProfile(locked) => {
                self.locked = locked;
                self.check_locked.set_active(locked);
                self.check_normal.set_active(!locked);

                let args_flag = format!(
                    "--args=asus_wmi.fnlock_default={}",
                    if locked { "0" } else { "1" }
                );
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            let result = run_command_blocking(
                                "pkexec",
                                &[
                                    "grubby",
                                    "--update-kernel=ALL",
                                    "--remove-args=asus_wmi.fnlock_default",
                                    &args_flag,
                                ],
                            )
                            .await;
                            match result {
                                Ok(()) => out.emit(FnKeyCommandOutput::Set(locked)),
                                Err(e) => out.emit(FnKeyCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            FnKeyMsg::ToggleLocked(locked) => {
                if locked == self.locked {
                    return;
                }
                self.locked = locked;

                let args_flag = format!(
                    "--args=asus_wmi.fnlock_default={}",
                    if locked { "0" } else { "1" }
                );

                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            let result = run_command_blocking(
                                "pkexec",
                                &[
                                    "grubby",
                                    "--update-kernel=ALL",
                                    "--remove-args=asus_wmi.fnlock_default",
                                    &args_flag,
                                ],
                            )
                            .await;

                            match result {
                                Ok(()) => out.emit(FnKeyCommandOutput::Set(locked)),
                                Err(e) => out.emit(FnKeyCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: FnKeyCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            FnKeyCommandOutput::GrubbyChecked(ok) => {
                self.grubby_available = ok;
                self.row_locked.set_sensitive(ok);
                self.row_normal.set_sensitive(ok);
            }
            FnKeyCommandOutput::Set(locked) => {
                AppConfig::update(|c| c.active_profile_mut().input_fn_key_locked = locked);
                let mode = if locked {
                    t!("fn_key_mode_locked")
                } else {
                    t!("fn_key_mode_normal")
                };
                self.row_hint.set_subtitle(&t!("fn_key_saved", mode = mode));
            }
            FnKeyCommandOutput::Error(e) => {
                self.row_hint
                    .set_subtitle(&t!("fn_key_save_error", error = e.clone()));
                let _ = sender.output(e);
            }
        }
    }
}
