use futures_util::StreamExt;
use gtk4 as gtk;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use tokio::sync::watch;

use crate::services::commands::run_command_blocking;
use crate::services::config::AppConfig;

// ──────────────────────────────────────────────────────────────────────────────
// iio-sensor-proxy D-Bus Proxy
// ──────────────────────────────────────────────────────────────────────────────

#[zbus::proxy(
    interface = "net.hadess.SensorProxy",
    default_service = "net.hadess.SensorProxy",
    default_path = "/net/hadess/SensorProxy"
)]
trait SensorProxy {
    fn claim_light(&self) -> zbus::Result<()>;
    fn release_light(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn light_level(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn has_ambient_light(&self) -> zbus::Result<bool>;
}

// ──────────────────────────────────────────────────────────────────────────────
// Automatische Tastaturhintergrundbeleuchtung
// ──────────────────────────────────────────────────────────────────────────────

pub struct AutoBeleuchtungModel {
    aufhellung_aktiv: bool,
    abdunklung_aktiv: bool,
    aufhellung_schwelle: f64,
    abdunklung_schwelle: f64,
    loop_tx: Option<watch::Sender<bool>>,
    aktuelle_lux: Option<f64>,
}

#[derive(Debug)]
pub enum AutoBeleuchtungMsg {
    AufhellungUmschalten(bool),
    AbdunklungUmschalten(bool),
    AufhellungSchwelleGeaendert(f64),
    AbdunklungSchwelleGeaendert(f64),
}

#[derive(Debug)]
pub enum AutoBeleuchtungOutput {
    Fehler(String),
    LuxAktualisiert(f64),
}

#[relm4::component(pub)]
impl Component for AutoBeleuchtungModel {
    type Init = ();
    type Input = AutoBeleuchtungMsg;
    type Output = String;
    type CommandOutput = AutoBeleuchtungOutput;

    view! {
        adw::PreferencesGroup {
            set_title: "Automatische Tastaturhintergrundbeleuchtung",
            set_description: Some("Passt die Tastaturhintergrundbeleuchtung automatisch an das Umgebungslicht an. Erfordert iio-sensor-proxy."),

            add = &adw::ActionRow {
                set_title: "Aktueller Lichtwert",
                set_subtitle: "Live-Messwert des Umgebungslichtsensors",

                #[watch]
                set_visible: model.aufhellung_aktiv || model.abdunklung_aktiv,

                add_suffix = &gtk::Label {
                    #[watch]
                    set_label: &match model.aktuelle_lux {
                        Some(lux) => format!("{lux:.1} lx"),
                        None => "— lx".to_string(),
                    },
                    add_css_class: "numeric",
                    set_valign: gtk::Align::Center,
                },
            },

            add = &adw::SwitchRow {
                set_title: "Automatische Aufhellung",
                set_subtitle: "Schaltet die Hintergrundbeleuchtung automatisch ein, wenn es dunkel ist.",

                #[watch]
                set_active: model.aufhellung_aktiv,

                connect_active_notify[sender] => move |switch| {
                    sender.input(AutoBeleuchtungMsg::AufhellungUmschalten(switch.is_active()));
                },
            },

            add = &adw::ActionRow {
                set_title: "Schwellenwert für Aufhellung (Lux)",
                set_subtitle: "Lichtlevel, unter dem die Tastatur leuchtet",

                #[watch]
                set_sensitive: model.aufhellung_aktiv,

                add_suffix = &gtk::SpinButton::with_range(0.0, 1000.0, 1.0) {
                    set_valign: gtk::Align::Center,

                    #[watch]
                    set_value: model.aufhellung_schwelle,

                    connect_value_changed[sender] => move |spin| {
                        sender.input(AutoBeleuchtungMsg::AufhellungSchwelleGeaendert(spin.value()));
                    },
                },
            },

            add = &adw::SwitchRow {
                set_title: "Automatische Abdunklung",
                set_subtitle: "Schaltet die Hintergrundbeleuchtung automatisch aus, wenn es hell ist.",

                #[watch]
                set_active: model.abdunklung_aktiv,

                connect_active_notify[sender] => move |switch| {
                    sender.input(AutoBeleuchtungMsg::AbdunklungUmschalten(switch.is_active()));
                },
            },

            add = &adw::ActionRow {
                set_title: "Schwellenwert für Abdunklung (Lux)",
                set_subtitle: "Lichtlevel, über dem die Tastatur ausgeht",

                #[watch]
                set_sensitive: model.abdunklung_aktiv,

                add_suffix = &gtk::SpinButton::with_range(0.0, 1000.0, 1.0) {
                    set_valign: gtk::Align::Center,

                    #[watch]
                    set_value: model.abdunklung_schwelle,

                    connect_value_changed[sender] => move |spin| {
                        sender.input(AutoBeleuchtungMsg::AbdunklungSchwelleGeaendert(spin.value()));
                    },
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
        let aufhellung = config.kbd_aufhellung_aktiv;
        let abdunklung = config.kbd_abdunklung_aktiv;
        let aufhellung_schwelle = config.kbd_aufhellung_schwelle;
        let abdunklung_schwelle = config.kbd_abdunklung_schwelle;

        let loop_tx = if aufhellung || abdunklung {
            Some(start_sensor_loop(
                aufhellung,
                aufhellung_schwelle,
                abdunklung,
                abdunklung_schwelle,
                &sender,
            ))
        } else {
            None
        };

        let model = AutoBeleuchtungModel {
            aufhellung_aktiv: aufhellung,
            abdunklung_aktiv: abdunklung,
            aufhellung_schwelle,
            abdunklung_schwelle,
            loop_tx,
            aktuelle_lux: None,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(
        &mut self,
        msg: AutoBeleuchtungMsg,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AutoBeleuchtungMsg::AufhellungUmschalten(aktiv) => {
                self.aufhellung_aktiv = aktiv;
                AppConfig::update(|c| c.kbd_aufhellung_aktiv = aktiv);
                self.sensor_loop_aktualisieren(sender);
            }
            AutoBeleuchtungMsg::AbdunklungUmschalten(aktiv) => {
                self.abdunklung_aktiv = aktiv;
                AppConfig::update(|c| c.kbd_abdunklung_aktiv = aktiv);
                self.sensor_loop_aktualisieren(sender);
            }
            AutoBeleuchtungMsg::AufhellungSchwelleGeaendert(wert) => {
                if (wert - self.aufhellung_schwelle).abs() > f64::EPSILON {
                    self.aufhellung_schwelle = wert;
                    AppConfig::update(|c| c.kbd_aufhellung_schwelle = wert);
                    self.sensor_loop_aktualisieren(sender);
                }
            }
            AutoBeleuchtungMsg::AbdunklungSchwelleGeaendert(wert) => {
                if (wert - self.abdunklung_schwelle).abs() > f64::EPSILON {
                    self.abdunklung_schwelle = wert;
                    AppConfig::update(|c| c.kbd_abdunklung_schwelle = wert);
                    self.sensor_loop_aktualisieren(sender);
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: AutoBeleuchtungOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AutoBeleuchtungOutput::Fehler(e) => {
                let _ = sender.output(e);
            }
            AutoBeleuchtungOutput::LuxAktualisiert(lux) => {
                self.aktuelle_lux = Some(lux);
            }
        }
    }
}

impl AutoBeleuchtungModel {
    fn sensor_loop_aktualisieren(&mut self, sender: ComponentSender<Self>) {
        let aktiv = self.aufhellung_aktiv || self.abdunklung_aktiv;

        if aktiv {
            // Stoppe vorherigen Loop falls vorhanden
            if let Some(tx) = &self.loop_tx {
                let _ = tx.send(false);
            }
            self.loop_tx = Some(start_sensor_loop(
                self.aufhellung_aktiv,
                self.aufhellung_schwelle,
                self.abdunklung_aktiv,
                self.abdunklung_schwelle,
                &sender,
            ));
        } else {
            if let Some(tx) = self.loop_tx.take() {
                let _ = tx.send(false);
            }
            self.aktuelle_lux = None;
        }
    }
}

async fn kbd_helligkeit_setzen(wert: i32) -> bool {
    run_command_blocking(
        "busctl",
        &[
            "call",
            "--system",
            "org.freedesktop.UPower",
            "/org/freedesktop/UPower/KbdBacklight",
            "org.freedesktop.UPower.KbdBacklight",
            "SetBrightness",
            "i",
            &wert.to_string(),
        ],
    )
    .await
    .is_ok()
}

async fn lichtsensor_logik(
    level: f64,
    aufhellung: bool,
    aufhellung_schwelle: f64,
    abdunklung: bool,
    abdunklung_schwelle: f64,
    mut aktuelle_helligkeit: i32,
) -> i32 {
    if aufhellung && level < aufhellung_schwelle && aktuelle_helligkeit != 3 {
        if kbd_helligkeit_setzen(3).await {
            aktuelle_helligkeit = 3;
        }
    } else if abdunklung && level > abdunklung_schwelle && aktuelle_helligkeit != 0 {
        if kbd_helligkeit_setzen(0).await {
            aktuelle_helligkeit = 0;
        }
    }
    aktuelle_helligkeit
}

fn start_sensor_loop(
    aufhellung: bool,
    aufhellung_schwelle: f64,
    abdunklung: bool,
    abdunklung_schwelle: f64,
    sender: &ComponentSender<AutoBeleuchtungModel>,
) -> watch::Sender<bool> {
    let (tx, mut rx) = watch::channel(true);
    let out = sender.command_sender().clone();

    tokio::spawn(async move {
        let conn = match zbus::Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                out.emit(AutoBeleuchtungOutput::Fehler(format!(
                    "D-Bus Verbindung fehlgeschlagen: {e}"
                )));
                return;
            }
        };

        let proxy = match SensorProxyProxy::new(&conn).await {
            Ok(p) => p,
            Err(e) => {
                out.emit(AutoBeleuchtungOutput::Fehler(format!(
                    "SensorProxy fehlgeschlagen: {e}"
                )));
                return;
            }
        };

        if let Err(e) = proxy.claim_light().await {
            out.emit(AutoBeleuchtungOutput::Fehler(format!(
                "claim_light fehlgeschlagen: {e}"
            )));
            return;
        }

        let level_stream = proxy.receive_light_level_changed().await;
        let mut aktuelle_helligkeit: i32 = -1;
        let mut letztes_level: f64 = -100.0;

        // Startwert einmalig auslesen und Logik anwenden
        match proxy.light_level().await {
            Ok(level) => {
                letztes_level = level;
                aktuelle_helligkeit = lichtsensor_logik(
                    level,
                    aufhellung,
                    aufhellung_schwelle,
                    abdunklung,
                    abdunklung_schwelle,
                    aktuelle_helligkeit,
                )
                .await;
                out.emit(AutoBeleuchtungOutput::LuxAktualisiert(level));
            }
            Err(e) => eprintln!("Startwert LightLevel fehlgeschlagen: {e}"),
        }

        tokio::pin!(level_stream);

        loop {
            tokio::select! {
                _ = rx.changed() => {
                    if !*rx.borrow() {
                        break;
                    }
                }
                maybe = level_stream.next() => {
                    if let Some(changed) = maybe {
                        match changed.get().await {
                            Ok(level) => {
                                if (level - letztes_level).abs() < 3.0 {
                                    continue;
                                }
                                letztes_level = level;
                                aktuelle_helligkeit = lichtsensor_logik(
                                    level,
                                    aufhellung,
                                    aufhellung_schwelle,
                                    abdunklung,
                                    abdunklung_schwelle,
                                    aktuelle_helligkeit,
                                )
                                .await;
                                out.emit(AutoBeleuchtungOutput::LuxAktualisiert(level));
                            }
                            Err(e) => eprintln!("LightLevel lesen fehlgeschlagen: {e}"),
                        }
                    } else {
                        // Stream beendet
                        break;
                    }
                }
            }
        }

        let _ = proxy.release_light().await;
    });

    tx
}

// ──────────────────────────────────────────────────────────────────────────────
// Ruhezustand der Tastaturhintergrundbeleuchtung
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum TimeoutModus {
    #[default]
    Nichts,
    AkkuUndNetz,
    NurAkku,
}

impl From<u32> for TimeoutModus {
    fn from(v: u32) -> Self {
        match v {
            1 => Self::AkkuUndNetz,
            2 => Self::NurAkku,
            _ => Self::Nichts,
        }
    }
}

const TIMEOUT_SEKUNDEN: [u32; 3] = [60, 120, 300];

fn busctl_brightness_cmd(wert: i32, nur_akku: bool) -> String {
    let base = format!(
        "busctl call --system org.freedesktop.UPower \
         /org/freedesktop/UPower/KbdBacklight \
         org.freedesktop.UPower.KbdBacklight SetBrightness i {wert}"
    );
    if nur_akku {
        format!(
            "if [ \"$(cat /sys/class/power_supply/*/online | head -n1)\" = \"0\" ]; \
             then {base}; fi"
        )
    } else {
        base
    }
}

pub struct RuhezustandModel {
    timeout_modus: TimeoutModus,
    check_nichts: gtk::CheckButton,
    check_akku_netz: gtk::CheckButton,
    check_nur_akku: gtk::CheckButton,
    dropdown_akku_netz: gtk::DropDown,
    dropdown_nur_akku: gtk::DropDown,
    swayidle_task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
pub enum RuhezustandMsg {
    ModusWechseln(TimeoutModus),
    AkkuNetzZeitGeaendert(u32),
    NurAkkuZeitGeaendert(u32),
}

#[derive(Debug)]
pub enum RuhezustandOutput {
    Fehler(String),
}

#[relm4::component(pub)]
impl Component for RuhezustandModel {
    type Init = ();
    type Input = RuhezustandMsg;
    type Output = String;
    type CommandOutput = RuhezustandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: "Ruhezustand der Tastaturhintergrundbeleuchtung",
            set_description: Some("Legt fest, wann die Tastaturhintergrundbeleuchtung in den Ruhezustand versetzt wird."),

            add = &adw::ActionRow {
                set_title: "Nichts wird ausgeführt",
                set_subtitle: "Tastaturhintergrundbeleuchtung immer aktiviert lassen",
                add_prefix = &model.check_nichts.clone(),
                set_activatable_widget: Some(&model.check_nichts),
            },

            add = &adw::ActionRow {
                set_title: "Im Akkubetrieb oder im Netzteilbetrieb",
                add_prefix = &model.check_akku_netz.clone(),
                set_activatable_widget: Some(&model.check_akku_netz),
                add_suffix = &model.dropdown_akku_netz.clone() -> gtk::DropDown {
                    set_valign: gtk::Align::Center,
                    #[watch]
                    set_sensitive: model.timeout_modus == TimeoutModus::AkkuUndNetz,
                },
            },

            add = &adw::ActionRow {
                set_title: "Nur im Akkubetrieb",
                add_prefix = &model.check_nur_akku.clone(),
                set_activatable_widget: Some(&model.check_nur_akku),
                add_suffix = &model.dropdown_nur_akku.clone() -> gtk::DropDown {
                    set_valign: gtk::Align::Center,
                    #[watch]
                    set_sensitive: model.timeout_modus == TimeoutModus::NurAkku,
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
        let modus = TimeoutModus::from(config.kbd_timeout_modus);

        let check_nichts = gtk::CheckButton::new();
        let check_akku_netz = gtk::CheckButton::new();
        let check_nur_akku = gtk::CheckButton::new();
        check_akku_netz.set_group(Some(&check_nichts));
        check_nur_akku.set_group(Some(&check_nichts));

        match modus {
            TimeoutModus::Nichts => check_nichts.set_active(true),
            TimeoutModus::AkkuUndNetz => check_akku_netz.set_active(true),
            TimeoutModus::NurAkku => check_nur_akku.set_active(true),
        }

        let zeitoptionen = gtk::StringList::new(&["1 Minute(n)", "2 Minute(n)", "5 Minute(n)"]);
        let dropdown_akku_netz =
            gtk::DropDown::new(Some(zeitoptionen.clone()), gtk::Expression::NONE);
        let dropdown_nur_akku = gtk::DropDown::new(Some(zeitoptionen), gtk::Expression::NONE);
        dropdown_akku_netz.set_selected(config.kbd_timeout_akku_netz_index);
        dropdown_nur_akku.set_selected(config.kbd_timeout_nur_akku_index);

        for (btn, modus_val) in [
            (&check_nichts, TimeoutModus::Nichts),
            (&check_akku_netz, TimeoutModus::AkkuUndNetz),
            (&check_nur_akku, TimeoutModus::NurAkku),
        ] {
            let sender = sender.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(RuhezustandMsg::ModusWechseln(modus_val));
                }
            });
        }

        {
            let sender = sender.clone();
            dropdown_akku_netz.connect_selected_notify(move |dd| {
                sender.input(RuhezustandMsg::AkkuNetzZeitGeaendert(dd.selected()));
            });
        }
        {
            let sender = sender.clone();
            dropdown_nur_akku.connect_selected_notify(move |dd| {
                sender.input(RuhezustandMsg::NurAkkuZeitGeaendert(dd.selected()));
            });
        }

        let mut model = RuhezustandModel {
            timeout_modus: modus,
            check_nichts,
            check_akku_netz,
            check_nur_akku,
            dropdown_akku_netz,
            dropdown_nur_akku,
            swayidle_task: None,
        };

        let widgets = view_output!();
        model.timeout_schreiben(modus, &sender);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: RuhezustandMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            RuhezustandMsg::ModusWechseln(modus) => {
                self.timeout_modus = modus;
                AppConfig::update(|c| c.kbd_timeout_modus = modus as u32);
                self.timeout_schreiben(modus, &sender);
            }
            RuhezustandMsg::AkkuNetzZeitGeaendert(index) => {
                AppConfig::update(|c| c.kbd_timeout_akku_netz_index = index);
                if self.timeout_modus == TimeoutModus::AkkuUndNetz {
                    self.timeout_schreiben(TimeoutModus::AkkuUndNetz, &sender);
                }
            }
            RuhezustandMsg::NurAkkuZeitGeaendert(index) => {
                AppConfig::update(|c| c.kbd_timeout_nur_akku_index = index);
                if self.timeout_modus == TimeoutModus::NurAkku {
                    self.timeout_schreiben(TimeoutModus::NurAkku, &sender);
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: RuhezustandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            RuhezustandOutput::Fehler(e) => {
                let _ = sender.output(e);
            }
        }
    }
}

impl RuhezustandModel {
    fn timeout_schreiben(
        &mut self,
        modus: TimeoutModus,
        sender: &ComponentSender<RuhezustandModel>,
    ) {
        if let Some(task) = self.swayidle_task.take() {
            task.abort();
        }

        if modus == TimeoutModus::Nichts {
            return;
        }

        let sekunden = match modus {
            TimeoutModus::Nichts => unreachable!(),
            TimeoutModus::AkkuUndNetz => {
                let idx = self.dropdown_akku_netz.selected() as usize;
                *TIMEOUT_SEKUNDEN.get(idx).unwrap_or(&60)
            }
            TimeoutModus::NurAkku => {
                let idx = self.dropdown_nur_akku.selected() as usize;
                *TIMEOUT_SEKUNDEN.get(idx).unwrap_or(&60)
            }
        };

        let nur_akku = modus == TimeoutModus::NurAkku;
        let timeout_cmd = busctl_brightness_cmd(0, nur_akku);
        let resume_cmd = busctl_brightness_cmd(3, nur_akku);
        let sekunden_str = sekunden.to_string();

        let cmd_sender = sender.command_sender().clone();
        let handle = tokio::spawn(async move {
            let mut child = match tokio::process::Command::new("swayidle")
                .kill_on_drop(true)
                .args([
                    "-w",
                    "timeout",
                    &sekunden_str,
                    &timeout_cmd,
                    "resume",
                    &resume_cmd,
                ])
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    cmd_sender.emit(RuhezustandOutput::Fehler(format!(
                        "swayidle starten fehlgeschlagen: {e}"
                    )));
                    return;
                }
            };
            if let Err(e) = child.wait().await {
                cmd_sender.emit(RuhezustandOutput::Fehler(format!(
                    "swayidle warten fehlgeschlagen: {e}"
                )));
            }
        });

        self.swayidle_task = Some(handle);
    }
}
