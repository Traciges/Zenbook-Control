use gtk4 as gtk;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::services::config::AppConfig;
use crate::services::dbus;
use crate::services::dbus::FanProfile;

pub struct FanModel {
    asusd_verfuegbar: bool,
    aktuelles_profil: FanProfile,
    check_leistung: gtk::CheckButton,
    check_standard: gtk::CheckButton,
    check_fluester: gtk::CheckButton,
}

#[derive(Debug)]
pub enum FanMsg {
    ProfilWechseln(FanProfile),
}

#[derive(Debug)]
pub enum FanCommandOutput {
    AsusdGeprueft(bool),
    ProfilGesetzt(FanProfile),
    Fehler(String),
}

#[relm4::component(pub)]
impl Component for FanModel {
    type Init = ();
    type Input = FanMsg;
    type Output = String;
    type CommandOutput = FanCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("fan_group_title"),
            set_description: Some(&t!("fan_group_desc")),

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

            add = &adw::ActionRow {
                set_title: &t!("fan_performance_title"),
                set_subtitle: &t!("fan_performance_subtitle"),
                add_prefix = &model.check_leistung.clone(),
                set_activatable_widget: Some(&model.check_leistung),
                #[watch]
                set_sensitive: model.asusd_verfuegbar,
            },

            add = &adw::ActionRow {
                set_title: &t!("fan_balanced_title"),
                set_subtitle: &t!("fan_balanced_subtitle"),
                add_prefix = &model.check_standard.clone(),
                set_activatable_widget: Some(&model.check_standard),
                #[watch]
                set_sensitive: model.asusd_verfuegbar,
            },

            add = &adw::ActionRow {
                set_title: &t!("fan_quiet_title"),
                set_subtitle: &t!("fan_quiet_subtitle"),
                add_prefix = &model.check_fluester.clone(),
                set_activatable_widget: Some(&model.check_fluester),
                #[watch]
                set_sensitive: model.asusd_verfuegbar,
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let check_leistung = gtk::CheckButton::new();
        let check_standard = gtk::CheckButton::new();
        let check_fluester = gtk::CheckButton::new();

        check_standard.set_group(Some(&check_leistung));
        check_fluester.set_group(Some(&check_leistung));

        let config = AppConfig::load();
        let gespeichertes_profil = FanProfile::from(config.fan_profil);
        match gespeichertes_profil {
            FanProfile::Performance => check_leistung.set_active(true),
            FanProfile::Balanced => check_standard.set_active(true),
            FanProfile::Quiet => check_fluester.set_active(true),
        }

        for (btn, profile) in [
            (&check_leistung, FanProfile::Performance),
            (&check_standard, FanProfile::Balanced),
            (&check_fluester, FanProfile::Quiet),
        ] {
            let sender = sender.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(FanMsg::ProfilWechseln(profile));
                }
            });
        }

        let model = FanModel {
            asusd_verfuegbar: false,
            aktuelles_profil: gespeichertes_profil,
            check_leistung,
            check_standard,
            check_fluester,
        };

        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    let verfuegbar = dbus::check_asusd_available().await;
                    out.emit(FanCommandOutput::AsusdGeprueft(verfuegbar));
                })
                .drop_on_shutdown()
        });

        sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    match dbus::get_fan_profile().await {
                        Ok(aktuell) if aktuell == gespeichertes_profil => {
                            out.emit(FanCommandOutput::ProfilGesetzt(aktuell));
                        }
                        Ok(_) => match dbus::set_fan_profile(gespeichertes_profil).await {
                            Ok(p) => out.emit(FanCommandOutput::ProfilGesetzt(p)),
                            Err(e) => out.emit(FanCommandOutput::Fehler(e)),
                        },
                        Err(e) => out.emit(FanCommandOutput::Fehler(e)),
                    }
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: FanMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            FanMsg::ProfilWechseln(profile) => {
                if profile == self.aktuelles_profil {
                    return;
                }
                self.aktuelles_profil = profile;
                AppConfig::update(|c| c.fan_profil = profile as u32);

                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus::set_fan_profile(profile).await {
                                Ok(p) => out.emit(FanCommandOutput::ProfilGesetzt(p)),
                                Err(e) => out.emit(FanCommandOutput::Fehler(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: FanCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            FanCommandOutput::AsusdGeprueft(verfuegbar) => {
                self.asusd_verfuegbar = verfuegbar;
            }
            FanCommandOutput::ProfilGesetzt(profile) => {
                eprintln!(
                    "{}",
                    t!("fan_profile_set", profile = format!("{:?}", profile))
                );
            }
            FanCommandOutput::Fehler(e) => {
                let _ = sender.output(e);
            }
        }
    }
}
