use gtk4 as gtk;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;

use super::helpers::{
    icc_profil_anwenden, icc_profil_zuruecksetzen, kwriteconfig_ausfuehren, qdbus_ausfuehren,
};
use crate::services::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SplendidProfil {
    #[default]
    Normal,
    Lebendig,
    Manuell,
    EyeCare,
    EReading,
}

pub struct SplendidModel {
    aktuelles_profil: SplendidProfil,
    farbtemperatur: f64,
    eye_care_staerke: f64,
    check_normal: gtk::CheckButton,
    check_lebendig: gtk::CheckButton,
    check_manuell: gtk::CheckButton,
    check_eye_care: gtk::CheckButton,
    check_e_reading: gtk::CheckButton,
    scale_farbtemperatur: gtk::Scale,
    scale_eye_care: gtk::Scale,
}

#[derive(Debug)]
pub enum SplendidMsg {
    ProfilWechseln(SplendidProfil),
    FarbtemperaturGeaendert(f64),
    EyeCareStaerkeGeaendert(f64),
}

#[derive(Debug)]
pub enum SplendidCommandOutput {
    EyeCareGesetzt(bool),
    FarbtemperaturGesetzt(u32),
    ProfilAngewendet(SplendidProfil),
    Fehler(String),
}

#[relm4::component(pub)]
impl Component for SplendidModel {
    type Init = ();
    type Input = SplendidMsg;
    type Output = ();
    type CommandOutput = SplendidCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: "Splendid",

            add = &adw::ActionRow {
                set_title: "Normal",
                add_prefix = &model.check_normal.clone(),
                set_activatable_widget: Some(&model.check_normal),
            },

            add = &adw::ActionRow {
                set_title: "Lebendig",
                add_prefix = &model.check_lebendig.clone(),
                set_activatable_widget: Some(&model.check_lebendig),
            },

            add = &adw::ActionRow {
                set_title: "Manuell",
                add_prefix = &model.check_manuell.clone(),
                set_activatable_widget: Some(&model.check_manuell),
            },

            add = &adw::ActionRow {
                set_title: "Farbtemperatur",
                add_suffix = &model.scale_farbtemperatur.clone(),

                #[watch]
                set_visible: model.aktuelles_profil == SplendidProfil::Manuell,
            },

            add = &adw::ActionRow {
                set_title: "Eye Care",
                add_prefix = &model.check_eye_care.clone(),
                set_activatable_widget: Some(&model.check_eye_care),
            },

            add = &adw::ActionRow {
                set_title: "Stärke",
                add_suffix = &model.scale_eye_care.clone(),

                #[watch]
                set_visible: model.aktuelles_profil == SplendidProfil::EyeCare,
            },

            add = &adw::ActionRow {
                set_title: "E-Reading",
                set_subtitle: "Graustufen",
                add_prefix = &model.check_e_reading.clone(),
                set_activatable_widget: Some(&model.check_e_reading),
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let check_normal = gtk::CheckButton::new();
        let check_lebendig = gtk::CheckButton::new();
        let check_manuell = gtk::CheckButton::new();
        let check_eye_care = gtk::CheckButton::new();
        let check_e_reading = gtk::CheckButton::new();

        check_lebendig.set_group(Some(&check_normal));
        check_manuell.set_group(Some(&check_normal));
        check_eye_care.set_group(Some(&check_normal));
        check_e_reading.set_group(Some(&check_normal));
        check_normal.set_active(true);

        for (btn, profil) in [
            (&check_normal, SplendidProfil::Normal),
            (&check_lebendig, SplendidProfil::Lebendig),
            (&check_manuell, SplendidProfil::Manuell),
            (&check_eye_care, SplendidProfil::EyeCare),
            (&check_e_reading, SplendidProfil::EReading),
        ] {
            let sender = sender.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(SplendidMsg::ProfilWechseln(profil));
                }
            });
        }

        let scale_farbtemperatur =
            gtk::Scale::with_range(gtk::Orientation::Horizontal, 2000.0, 6500.0, 100.0);
        scale_farbtemperatur.set_hexpand(true);
        scale_farbtemperatur.set_width_request(300);
        scale_farbtemperatur.set_valign(gtk::Align::Center);

        let scale_eye_care = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        scale_eye_care.set_hexpand(true);
        scale_eye_care.set_width_request(300);
        scale_eye_care.set_valign(gtk::Align::Center);

        {
            let sender = sender.clone();
            scale_farbtemperatur.connect_value_changed(move |s| {
                sender.input(SplendidMsg::FarbtemperaturGeaendert(s.value()));
            });
        }
        {
            let sender = sender.clone();
            scale_eye_care.connect_value_changed(move |s| {
                sender.input(SplendidMsg::EyeCareStaerkeGeaendert(s.value()));
            });
        }

        // Gespeicherten Zustand wiederherstellen
        let config = AppConfig::load();
        let gespeichertes_profil = config.splendid_profil;

        scale_farbtemperatur.set_value(config.farbtemperatur);
        scale_eye_care.set_value(config.eye_care_staerke);

        match gespeichertes_profil {
            SplendidProfil::Normal => {}
            SplendidProfil::Lebendig => check_lebendig.set_active(true),
            SplendidProfil::Manuell => check_manuell.set_active(true),
            SplendidProfil::EyeCare => check_eye_care.set_active(true),
            SplendidProfil::EReading => check_e_reading.set_active(true),
        }

        let model = SplendidModel {
            aktuelles_profil: gespeichertes_profil,
            farbtemperatur: config.farbtemperatur,
            eye_care_staerke: config.eye_care_staerke,
            check_normal,
            check_lebendig,
            check_manuell,
            check_eye_care,
            check_e_reading,
            scale_farbtemperatur,
            scale_eye_care,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: SplendidMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            SplendidMsg::ProfilWechseln(profil) => {
                if profil == self.aktuelles_profil {
                    return;
                }
                let vorheriges = self.aktuelles_profil;
                self.aktuelles_profil = profil;

                AppConfig::update(|c| c.splendid_profil = profil);

                // Night Color deaktivieren, wenn wir Eye Care verlassen
                if vorheriges == SplendidProfil::EyeCare && profil != SplendidProfil::EyeCare {
                    sender.command(|out, shutdown| {
                        shutdown
                            .register(async move {
                                night_color_setzen(false, &out).await;
                            })
                            .drop_on_shutdown()
                    });
                }

                match profil {
                    SplendidProfil::EyeCare => {
                        sender.command(|out, shutdown| {
                            shutdown
                                .register(async move {
                                    night_color_setzen(true, &out).await;
                                })
                                .drop_on_shutdown()
                        });
                    }
                    SplendidProfil::Normal => {
                        sender.command(|out, shutdown| {
                            shutdown
                                .register(async move {
                                    match icc_profil_zuruecksetzen().await {
                                        Ok(()) => {
                                            out.emit(SplendidCommandOutput::ProfilAngewendet(
                                                SplendidProfil::Normal,
                                            ))
                                        }
                                        Err(e) => out.emit(SplendidCommandOutput::Fehler(e)),
                                    }
                                })
                                .drop_on_shutdown()
                        });
                    }
                    SplendidProfil::Lebendig => {
                        sender.command(|out, shutdown| {
                            shutdown
                                .register(async move {
                                    match icc_profil_anwenden("lebendig.icc").await {
                                        Ok(()) => {
                                            out.emit(SplendidCommandOutput::ProfilAngewendet(
                                                SplendidProfil::Lebendig,
                                            ))
                                        }
                                        Err(e) => out.emit(SplendidCommandOutput::Fehler(e)),
                                    }
                                })
                                .drop_on_shutdown()
                        });
                    }
                    SplendidProfil::Manuell => {
                        eprintln!(
                            "Splendid: Manuell-Profil aktiviert – Farbtemperatur über Slider einstellen"
                        );
                    }
                    SplendidProfil::EReading => {
                        sender.command(|out, shutdown| {
                            shutdown
                                .register(async move {
                                    match icc_profil_anwenden("ereading.icc").await {
                                        Ok(()) => {
                                            out.emit(SplendidCommandOutput::ProfilAngewendet(
                                                SplendidProfil::EReading,
                                            ))
                                        }
                                        Err(e) => out.emit(SplendidCommandOutput::Fehler(e)),
                                    }
                                })
                                .drop_on_shutdown()
                        });
                    }
                }
            }
            SplendidMsg::FarbtemperaturGeaendert(wert) => {
                self.farbtemperatur = wert;

                AppConfig::update(|c| c.farbtemperatur = wert);

                let kelvin = wert as u32;

                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            farbtemperatur_setzen(kelvin, &out).await;
                        })
                        .drop_on_shutdown()
                });
            }
            SplendidMsg::EyeCareStaerkeGeaendert(wert) => {
                self.eye_care_staerke = wert;

                AppConfig::update(|c| c.eye_care_staerke = wert);

                eprintln!("Eye Care Stärke auf {} gesetzt", wert);
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: SplendidCommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            SplendidCommandOutput::EyeCareGesetzt(aktiv) => {
                eprintln!(
                    "Eye Care Night Color auf {} gesetzt",
                    if aktiv { "aktiv" } else { "inaktiv" }
                );
            }
            SplendidCommandOutput::FarbtemperaturGesetzt(kelvin) => {
                eprintln!("Farbtemperatur auf {}K gesetzt", kelvin);
            }
            SplendidCommandOutput::ProfilAngewendet(profil) => {
                eprintln!("Splendid: Profil {:?} angewendet", profil);
            }
            SplendidCommandOutput::Fehler(e) => {
                eprintln!("Fehler: {e}");
            }
        }
    }
}

/// Setzt die Farbtemperatur über kwriteconfig6 und toggelt Night Color zum Neuladen.
async fn farbtemperatur_setzen(kelvin: u32, out: &relm4::Sender<SplendidCommandOutput>) {
    let kelvin_str = kelvin.to_string();
    if let Err(e) = kwriteconfig_ausfuehren(&[
        "--file",
        "kwinrc",
        "--group",
        "NightColor",
        "--key",
        "NightTemperature",
        &kelvin_str,
    ])
    .await
    {
        out.emit(SplendidCommandOutput::Fehler(e));
        return;
    }
    kwin_reconfigure(out).await;
    out.emit(SplendidCommandOutput::FarbtemperaturGesetzt(kelvin));
}

/// Setzt Night Color an/aus via kwriteconfig6 + KWin reconfigure (KDE Plasma 6 Wayland).
async fn night_color_setzen(aktiv: bool, out: &relm4::Sender<SplendidCommandOutput>) {
    let wert = if aktiv { "true" } else { "false" };
    if let Err(e) = kwriteconfig_ausfuehren(&[
        "--file",
        "kwinrc",
        "--group",
        "NightColor",
        "--key",
        "Active",
        wert,
    ])
    .await
    {
        out.emit(SplendidCommandOutput::Fehler(e));
        return;
    }
    kwin_reconfigure(out).await;
    out.emit(SplendidCommandOutput::EyeCareGesetzt(aktiv));
}

/// Sendet org.kde.KWin.reconfigure, damit kwinrc-Änderungen sofort wirksam werden.
async fn kwin_reconfigure(out: &relm4::Sender<SplendidCommandOutput>) {
    let args = vec![
        "org.kde.KWin".to_string(),
        "/KWin".to_string(),
        "org.kde.KWin.reconfigure".to_string(),
    ];
    if let Err(e) = qdbus_ausfuehren(args).await {
        out.emit(SplendidCommandOutput::Fehler(e));
    }
}
