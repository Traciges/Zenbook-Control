use gtk4 as gtk;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::services::commands::run_command_blocking;
use crate::services::config::AppConfig;

pub struct FnKeyModel {
    locked: bool,
    row_hint: adw::ActionRow,
    row_locked: adw::ActionRow,
    row_normal: adw::ActionRow,
}

#[derive(Debug)]
pub enum FnKeyMsg {
    ToggleLocked(bool),
}

#[derive(Debug)]
pub enum FnKeyCommandOutput {
    Set(bool),
    Fehler(String),
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

        let locked = AppConfig::load().input_fn_key_gesperrt;
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
            row_hint,
            row_locked,
            row_normal,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: FnKeyMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
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
                                Err(e) => out.emit(FnKeyCommandOutput::Fehler(e)),
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
            FnKeyCommandOutput::Set(locked) => {
                AppConfig::update(|c| c.input_fn_key_gesperrt = locked);
                let mode = if locked {
                    t!("fn_key_mode_locked")
                } else {
                    t!("fn_key_mode_normal")
                };
                self.row_hint
                    .set_subtitle(&t!("fn_key_saved", mode = mode));
            }
            FnKeyCommandOutput::Fehler(e) => {
                self.row_hint
                    .set_subtitle(&t!("fn_key_save_error", error = e.clone()));
                let _ = sender.output(e);
            }
        }
    }
}
