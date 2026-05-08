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

use std::collections::BTreeMap;

use gtk4 as gtk;
use gtk4::gdk::RGBA;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::services::config::AppConfig;
use crate::services::dbus::{
    self, AuraDeviceInfo, AuraEffect, AuraModeNum, AuraPowerState, AuraZone, Colour,
    LaptopAuraPower, PowerZones,
};

const SPEED_VALUES: &[&str] = &["Low", "Med", "High"];
const SPEED_KEYS: &[&str] = &["aura_speed_low", "aura_speed_med", "aura_speed_high"];
const DIRECTION_VALUES: &[&str] = &["Right", "Left", "Up", "Down"];
const DIRECTION_KEYS: &[&str] = &[
    "aura_direction_right",
    "aura_direction_left",
    "aura_direction_up",
    "aura_direction_down",
];
const BRIGHTNESS_KEYS: &[&str] = &[
    "aura_brightness_off",
    "aura_brightness_low",
    "aura_brightness_med",
    "aura_brightness_high",
];

/// Per-device controller for one `xyz.ljones.Aura` object.
pub struct AuraDeviceModel {
    info: AuraDeviceInfo,
    initialised: bool,

    supported_modes: Vec<AuraModeNum>,
    supported_zones: Vec<AuraZone>,
    supported_brightness_levels: Vec<u32>,
    supported_power_zones: Vec<PowerZones>,

    current_mode: AuraModeNum,
    current_zone: AuraZone,
    current_brightness: u32,
    current_colour1: Colour,
    current_colour2: Colour,
    current_speed: String,
    current_direction: String,
    /// Per-mode effect cache so flipping modes restores the user's last
    /// settings (mirrors rog-control-center behaviour).
    effect_cache: BTreeMap<u32, AuraEffect>,

    power_state: LaptopAuraPower,

    // Imperatively-managed widgets.
    mode_combo: adw::ComboRow,
    zone_row: adw::ComboRow,
    brightness_combo: adw::ComboRow,
    colour1_row: adw::ActionRow,
    colour1_button: gtk::ColorDialogButton,
    colour2_row: adw::ActionRow,
    colour2_button: gtk::ColorDialogButton,
    speed_row: adw::ComboRow,
    direction_row: adw::ComboRow,
    power_expander: adw::ExpanderRow,
    power_switches: Vec<(PowerZones, gtk::Switch, gtk::Switch, gtk::Switch, gtk::Switch)>,
}

#[derive(Debug)]
pub enum AuraDeviceMsg {
    ChangeMode(u32),
    ChangeZone(u32),
    ChangeBrightness(u32),
    ChangeColour1(RGBA),
    ChangeColour2(RGBA),
    ChangeSpeed(u32),
    ChangeDirection(u32),
    PowerToggle(PowerZones, PowerField, bool),
    LoadProfile {
        mode: u32,
        zone: u32,
        brightness: u32,
        colour_r: u8,
        colour_g: u8,
        colour_b: u8,
        colour2_r: u8,
        colour2_g: u8,
        colour2_b: u8,
        speed: String,
        direction: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum PowerField {
    Boot,
    Awake,
    Sleep,
    Shutdown,
}

#[derive(Debug)]
pub enum AuraDeviceCmd {
    InitData {
        supported_modes: Vec<AuraModeNum>,
        supported_zones: Vec<AuraZone>,
        supported_brightness_levels: Vec<u32>,
        supported_power_zones: Vec<PowerZones>,
        current_effect: AuraEffect,
        brightness: u32,
        all_modes: BTreeMap<u32, AuraEffect>,
        power_state: LaptopAuraPower,
    },
    InitFailed(String),
    Persisted,
    Error(String),
}

#[relm4::component(pub)]
impl Component for AuraDeviceModel {
    type Init = AuraDeviceInfo;
    type Input = AuraDeviceMsg;
    type Output = String;
    type CommandOutput = AuraDeviceCmd;

    view! {
        adw::PreferencesGroup {
            #[watch]
            set_title: &t!(model.info.kind.i18n_key()),

            add = &model.mode_combo.clone() -> adw::ComboRow {
                set_title: &t!("aura_mode_title"),
                #[watch]
                set_sensitive: model.initialised,
            },

            add = &model.zone_row.clone() -> adw::ComboRow {
                set_title: &t!("aura_zone_title"),
                #[watch]
                set_visible: model.has_multiple_zones(),
                #[watch]
                set_sensitive: model.initialised,
            },

            add = &model.brightness_combo.clone() -> adw::ComboRow {
                set_title: &t!("aura_brightness_title"),
                #[watch]
                set_sensitive: model.initialised,
            },

            add = &model.colour1_row.clone() -> adw::ActionRow {
                set_title: &t!("aura_colour_title"),
                #[watch]
                set_sensitive: model.initialised && !model.current_mode.is_colour_irrelevant(),
            },

            add = &model.colour2_row.clone() -> adw::ActionRow {
                set_title: &t!("aura_colour2_title"),
                #[watch]
                set_visible: model.current_mode.uses_colour2(),
                #[watch]
                set_sensitive: model.initialised,
            },

            add = &model.speed_row.clone() -> adw::ComboRow {
                set_title: &t!("aura_speed_title"),
                #[watch]
                set_visible: model.current_mode.uses_speed(),
                #[watch]
                set_sensitive: model.initialised,
            },

            add = &model.direction_row.clone() -> adw::ComboRow {
                set_title: &t!("aura_direction_title"),
                #[watch]
                set_visible: model.current_mode.uses_direction(),
                #[watch]
                set_sensitive: model.initialised,
            },

            add = &model.power_expander.clone() -> adw::ExpanderRow {
                set_title: &t!("aura_power_title"),
                #[watch]
                set_visible: !model.supported_power_zones.is_empty(),
                #[watch]
                set_sensitive: model.initialised,
            },
        }
    }

    fn init(
        info: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mode_combo = adw::ComboRow::new();
        mode_combo.set_model(Some(&gtk::StringList::new(&[&t!("aura_mode_static")])));
        mode_combo.connect_selected_notify({
            let sender = sender.clone();
            move |row| sender.input(AuraDeviceMsg::ChangeMode(row.selected()))
        });

        let zone_row = adw::ComboRow::new();
        zone_row.set_model(Some(&gtk::StringList::new(&[&t!("aura_zone_none")])));
        zone_row.connect_selected_notify({
            let sender = sender.clone();
            move |row| sender.input(AuraDeviceMsg::ChangeZone(row.selected()))
        });

        let brightness_combo = adw::ComboRow::new();
        let bright_labels: Vec<String> = BRIGHTNESS_KEYS.iter().map(|k| t!(*k).to_string()).collect();
        let bright_refs: Vec<&str> = bright_labels.iter().map(|s| s.as_str()).collect();
        brightness_combo.set_model(Some(&gtk::StringList::new(&bright_refs)));
        brightness_combo.connect_selected_notify({
            let sender = sender.clone();
            move |row| sender.input(AuraDeviceMsg::ChangeBrightness(row.selected()))
        });

        let colour1_button = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
        colour1_button.set_valign(gtk::Align::Center);
        colour1_button.connect_rgba_notify({
            let sender = sender.clone();
            move |btn| sender.input(AuraDeviceMsg::ChangeColour1(btn.rgba()))
        });
        let colour1_row = adw::ActionRow::new();
        colour1_row.add_suffix(&colour1_button);

        let colour2_button = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
        colour2_button.set_valign(gtk::Align::Center);
        colour2_button.connect_rgba_notify({
            let sender = sender.clone();
            move |btn| sender.input(AuraDeviceMsg::ChangeColour2(btn.rgba()))
        });
        let colour2_row = adw::ActionRow::new();
        colour2_row.add_suffix(&colour2_button);

        let speed_row = adw::ComboRow::new();
        let speed_labels: Vec<String> = SPEED_KEYS.iter().map(|k| t!(*k).to_string()).collect();
        let speed_refs: Vec<&str> = speed_labels.iter().map(|s| s.as_str()).collect();
        speed_row.set_model(Some(&gtk::StringList::new(&speed_refs)));
        speed_row.connect_selected_notify({
            let sender = sender.clone();
            move |row| sender.input(AuraDeviceMsg::ChangeSpeed(row.selected()))
        });

        let direction_row = adw::ComboRow::new();
        let dir_labels: Vec<String> = DIRECTION_KEYS.iter().map(|k| t!(*k).to_string()).collect();
        let dir_refs: Vec<&str> = dir_labels.iter().map(|s| s.as_str()).collect();
        direction_row.set_model(Some(&gtk::StringList::new(&dir_refs)));
        direction_row.connect_selected_notify({
            let sender = sender.clone();
            move |row| sender.input(AuraDeviceMsg::ChangeDirection(row.selected()))
        });

        let power_expander = adw::ExpanderRow::new();

        let model = AuraDeviceModel {
            info: info.clone(),
            initialised: false,
            supported_modes: Vec::new(),
            supported_zones: vec![AuraZone::None],
            supported_brightness_levels: vec![0, 1, 2, 3],
            supported_power_zones: Vec::new(),
            current_mode: AuraModeNum::Static,
            current_zone: AuraZone::None,
            current_brightness: 2,
            current_colour1: Colour { r: 166, g: 0, b: 0 },
            current_colour2: Colour::BLACK,
            current_speed: "Med".to_string(),
            current_direction: "Right".to_string(),
            effect_cache: BTreeMap::new(),
            power_state: LaptopAuraPower { states: Vec::new() },
            mode_combo,
            zone_row,
            brightness_combo,
            colour1_row,
            colour1_button,
            colour2_row,
            colour2_button,
            speed_row,
            direction_row,
            power_expander,
            power_switches: Vec::new(),
        };

        let widgets = view_output!();

        let path = info.path.clone();
        sender.command(move |out, shutdown| {
            shutdown
                .register(async move { fetch_device_state(&path, out).await })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: AuraDeviceMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            AuraDeviceMsg::ChangeMode(idx) => {
                let Some(&mode) = self.supported_modes.get(idx as usize) else {
                    return;
                };
                if mode == self.current_mode {
                    return;
                }
                self.cache_current_effect();
                self.current_mode = mode;
                if let Some(cached) = self.effect_cache.get(&(mode as u32)).cloned() {
                    self.apply_effect_to_state(&cached);
                    self.sync_colour_widgets();
                    self.sync_speed_direction_widgets();
                }
                self.persist_keyboard_to_profile();
                self.send_effect(sender);
            }
            AuraDeviceMsg::ChangeZone(idx) => {
                let Some(&zone) = self.supported_zones.get(idx as usize) else {
                    return;
                };
                if zone == self.current_zone {
                    return;
                }
                self.current_zone = zone;
                self.persist_keyboard_to_profile();
                self.send_effect(sender);
            }
            AuraDeviceMsg::ChangeBrightness(idx) => {
                if idx == self.current_brightness {
                    return;
                }
                self.current_brightness = idx;
                self.persist_keyboard_to_profile();
                let path = self.info.path.clone();
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus::set_aura_brightness(&path, idx).await {
                                Ok(_) => out.emit(AuraDeviceCmd::Persisted),
                                Err(e) => out.emit(AuraDeviceCmd::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            AuraDeviceMsg::ChangeColour1(rgba) => {
                let c = rgba_to_colour(rgba);
                if c == self.current_colour1 {
                    return;
                }
                self.current_colour1 = c;
                self.persist_keyboard_to_profile();
                self.send_effect(sender);
            }
            AuraDeviceMsg::ChangeColour2(rgba) => {
                let c = rgba_to_colour(rgba);
                if c == self.current_colour2 {
                    return;
                }
                self.current_colour2 = c;
                self.persist_keyboard_to_profile();
                self.send_effect(sender);
            }
            AuraDeviceMsg::ChangeSpeed(idx) => {
                let Some(s) = SPEED_VALUES.get(idx as usize) else {
                    return;
                };
                if *s == self.current_speed.as_str() {
                    return;
                }
                self.current_speed = s.to_string();
                self.persist_keyboard_to_profile();
                self.send_effect(sender);
            }
            AuraDeviceMsg::ChangeDirection(idx) => {
                let Some(d) = DIRECTION_VALUES.get(idx as usize) else {
                    return;
                };
                if *d == self.current_direction.as_str() {
                    return;
                }
                self.current_direction = d.to_string();
                self.persist_keyboard_to_profile();
                self.send_effect(sender);
            }
            AuraDeviceMsg::PowerToggle(zone, field, value) => {
                if let Some(state) = self.power_state.states.iter_mut().find(|s| s.zone == zone) {
                    match field {
                        PowerField::Boot => state.boot = value,
                        PowerField::Awake => state.awake = value,
                        PowerField::Sleep => state.sleep = value,
                        PowerField::Shutdown => state.shutdown = value,
                    }
                }
                let path = self.info.path.clone();
                let power = self.power_state.clone();
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus::set_aura_led_power(&path, power).await {
                                Ok(()) => out.emit(AuraDeviceCmd::Persisted),
                                Err(e) => out.emit(AuraDeviceCmd::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            AuraDeviceMsg::LoadProfile {
                mode,
                zone,
                brightness,
                colour_r,
                colour_g,
                colour_b,
                colour2_r,
                colour2_g,
                colour2_b,
                speed,
                direction,
            } => {
                if !self.initialised {
                    return;
                }
                self.current_mode = AuraModeNum::from(mode);
                self.current_zone = AuraZone::from(zone);
                self.current_brightness = brightness.min(3);
                self.current_colour1 = Colour {
                    r: colour_r,
                    g: colour_g,
                    b: colour_b,
                };
                self.current_colour2 = Colour {
                    r: colour2_r,
                    g: colour2_g,
                    b: colour2_b,
                };
                self.current_speed = speed;
                self.current_direction = direction;

                self.sync_mode_widget();
                self.sync_zone_widget();
                self.brightness_combo.set_selected(self.current_brightness);
                self.sync_colour_widgets();
                self.sync_speed_direction_widgets();

                let path = self.info.path.clone();
                let effect = self.build_effect();
                let bright = self.current_brightness;
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            if let Err(e) = dbus::set_aura_effect(&path, effect).await {
                                out.emit(AuraDeviceCmd::Error(e));
                                return;
                            }
                            match dbus::set_aura_brightness(&path, bright).await {
                                Ok(_) => out.emit(AuraDeviceCmd::Persisted),
                                Err(e) => out.emit(AuraDeviceCmd::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: AuraDeviceCmd,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AuraDeviceCmd::InitData {
                supported_modes,
                supported_zones,
                supported_brightness_levels,
                supported_power_zones,
                current_effect,
                brightness,
                all_modes,
                power_state,
            } => {
                self.supported_modes = supported_modes;
                self.supported_zones = if supported_zones.is_empty() {
                    vec![AuraZone::None]
                } else {
                    supported_zones
                };
                self.supported_brightness_levels = supported_brightness_levels;
                self.supported_power_zones = supported_power_zones;
                self.effect_cache = all_modes;
                self.power_state = power_state;
                self.apply_effect_to_state(&current_effect);
                self.current_brightness = brightness.min(3);

                self.refresh_mode_model();
                self.refresh_zone_model();
                self.brightness_combo.set_selected(self.current_brightness);
                self.sync_colour_widgets();
                self.sync_speed_direction_widgets();
                self.rebuild_power_rows(sender);

                self.initialised = true;
            }
            AuraDeviceCmd::InitFailed(e) => {
                let _ = sender.output(e);
                // Keep widget tree visible-but-disabled. No toast — the page-level
                // component logs discovery failures.
                self.initialised = false;
            }
            AuraDeviceCmd::Persisted => {}
            AuraDeviceCmd::Error(e) => {
                let _ = sender.output(e);
            }
        }
    }
}

impl AuraDeviceModel {
    fn has_multiple_zones(&self) -> bool {
        self.supported_zones.len() > 1
    }

    fn build_effect(&self) -> AuraEffect {
        AuraEffect {
            mode: self.current_mode as u32,
            zone: self.current_zone as u32,
            colour1: self.current_colour1,
            colour2: self.current_colour2,
            speed: self.current_speed.clone(),
            direction: self.current_direction.clone(),
        }
    }

    fn cache_current_effect(&mut self) {
        let mode = self.current_mode as u32;
        self.effect_cache.insert(mode, self.build_effect());
    }

    fn apply_effect_to_state(&mut self, effect: &AuraEffect) {
        self.current_mode = AuraModeNum::from(effect.mode);
        self.current_zone = AuraZone::from(effect.zone);
        self.current_colour1 = effect.colour1;
        self.current_colour2 = effect.colour2;
        self.current_speed = effect.speed.clone();
        self.current_direction = effect.direction.clone();
    }

    fn send_effect(&self, sender: ComponentSender<Self>) {
        let path = self.info.path.clone();
        let effect = self.build_effect();
        sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    match dbus::set_aura_effect(&path, effect).await {
                        Ok(()) => out.emit(AuraDeviceCmd::Persisted),
                        Err(e) => out.emit(AuraDeviceCmd::Error(e)),
                    }
                })
                .drop_on_shutdown()
        });
    }

    fn refresh_mode_model(&self) {
        let labels: Vec<String> = self
            .supported_modes
            .iter()
            .map(|m| t!(m.i18n_key()).to_string())
            .collect();
        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        self.mode_combo.set_model(Some(&gtk::StringList::new(&refs)));
        self.sync_mode_widget();
    }

    fn refresh_zone_model(&self) {
        let labels: Vec<String> = self
            .supported_zones
            .iter()
            .map(|z| t!(z.i18n_key()).to_string())
            .collect();
        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        self.zone_row.set_model(Some(&gtk::StringList::new(&refs)));
        self.sync_zone_widget();
    }

    fn sync_mode_widget(&self) {
        if let Some(idx) = self
            .supported_modes
            .iter()
            .position(|&m| m == self.current_mode)
        {
            self.mode_combo.set_selected(idx as u32);
        }
    }

    fn sync_zone_widget(&self) {
        if let Some(idx) = self
            .supported_zones
            .iter()
            .position(|&z| z == self.current_zone)
        {
            self.zone_row.set_selected(idx as u32);
        }
    }

    fn sync_colour_widgets(&self) {
        self.colour1_button.set_rgba(&colour_to_rgba(self.current_colour1));
        self.colour2_button.set_rgba(&colour_to_rgba(self.current_colour2));
    }

    fn sync_speed_direction_widgets(&self) {
        if let Some(idx) = SPEED_VALUES.iter().position(|s| *s == self.current_speed) {
            self.speed_row.set_selected(idx as u32);
        }
        if let Some(idx) = DIRECTION_VALUES
            .iter()
            .position(|d| *d == self.current_direction)
        {
            self.direction_row.set_selected(idx as u32);
        }
    }

    fn rebuild_power_rows(&mut self, sender: ComponentSender<Self>) {
        // Clear existing children.
        for (_, b, a, s, sh) in self.power_switches.drain(..) {
            // ExpanderRow doesn't expose a child-removal API; widgets stay
            // attached to the underlying ListBox. In practice rebuild only
            // happens once at init, so leakage is bounded. Disable old switches
            // so stale toggles do nothing.
            for w in [&b, &a, &s, &sh] {
                w.set_sensitive(false);
            }
        }
        for zone in self.supported_power_zones.clone() {
            let state = self
                .power_state
                .states
                .iter()
                .find(|s| s.zone == zone)
                .copied()
                .unwrap_or(AuraPowerState {
                    zone,
                    boot: false,
                    awake: false,
                    sleep: false,
                    shutdown: false,
                });
            // Ensure power_state has an entry so toggles can mutate it.
            if !self.power_state.states.iter().any(|s| s.zone == zone) {
                self.power_state.states.push(state);
            }

            let row = adw::ActionRow::new();
            row.set_title(&t!(zone.i18n_key()));

            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            hbox.set_valign(gtk::Align::Center);

            let make_switch = |label_key: &str,
                               initial: bool,
                               field: PowerField,
                               sender: ComponentSender<Self>|
             -> (gtk::Box, gtk::Switch) {
                let cell = gtk::Box::new(gtk::Orientation::Vertical, 2);
                cell.set_valign(gtk::Align::Center);
                let lbl = gtk::Label::new(Some(&t!(label_key)));
                lbl.add_css_class("dim-label");
                lbl.add_css_class("caption");
                let sw = gtk::Switch::new();
                sw.set_active(initial);
                sw.set_valign(gtk::Align::Center);
                sw.connect_active_notify(move |sw| {
                    sender.input(AuraDeviceMsg::PowerToggle(zone, field, sw.is_active()));
                });
                cell.append(&lbl);
                cell.append(&sw);
                (cell, sw)
            };

            let (cb, sw_boot) = make_switch("aura_power_boot", state.boot, PowerField::Boot, sender.clone());
            let (ca, sw_awake) =
                make_switch("aura_power_awake", state.awake, PowerField::Awake, sender.clone());
            let (cs, sw_sleep) =
                make_switch("aura_power_sleep", state.sleep, PowerField::Sleep, sender.clone());
            let (csh, sw_shut) = make_switch(
                "aura_power_shutdown",
                state.shutdown,
                PowerField::Shutdown,
                sender.clone(),
            );

            hbox.append(&cb);
            hbox.append(&ca);
            hbox.append(&cs);
            hbox.append(&csh);
            row.add_suffix(&hbox);
            self.power_expander.add_row(&row);
            self.power_switches
                .push((zone, sw_boot, sw_awake, sw_sleep, sw_shut));
        }
    }

    /// Persists the current state into the active profile *if* this device is
    /// the primary keyboard. Lightbar / lid / anime devices do not feed the
    /// profile system today.
    fn persist_keyboard_to_profile(&self) {
        if !self.info.kind.is_keyboard() {
            return;
        }
        let mode = self.current_mode as u32;
        let zone = self.current_zone as u32;
        let bright = self.current_brightness;
        let c1 = self.current_colour1;
        let c2 = self.current_colour2;
        let speed = self.current_speed.clone();
        let direction = self.current_direction.clone();
        AppConfig::update(|c| {
            let p = c.active_profile_mut();
            p.aura_mode = mode;
            p.aura_zone = zone;
            p.aura_brightness = bright;
            p.aura_colour_r = c1.r;
            p.aura_colour_g = c1.g;
            p.aura_colour_b = c1.b;
            p.aura_colour2_r = c2.r;
            p.aura_colour2_g = c2.g;
            p.aura_colour2_b = c2.b;
            p.aura_speed = speed;
            p.aura_direction = direction;
        });
    }
}

fn colour_to_rgba(c: Colour) -> RGBA {
    RGBA::new(
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        1.0,
    )
}

fn rgba_to_colour(r: RGBA) -> Colour {
    Colour {
        r: (r.red() * 255.0) as u8,
        g: (r.green() * 255.0) as u8,
        b: (r.blue() * 255.0) as u8,
    }
}

async fn fetch_device_state(path: &str, out: relm4::Sender<AuraDeviceCmd>) {
    let supported_modes = match dbus::get_aura_supported_modes(path).await {
        Ok(v) => v,
        Err(e) => {
            out.emit(AuraDeviceCmd::InitFailed(e));
            return;
        }
    };
    let supported_modes: Vec<AuraModeNum> =
        supported_modes.into_iter().map(AuraModeNum::from).collect();

    let supported_zones: Vec<AuraZone> = dbus::get_aura_supported_zones(path)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(AuraZone::from)
        .collect();

    let supported_brightness = dbus::get_aura_supported_brightness(path)
        .await
        .unwrap_or_else(|_| vec![0, 1, 2, 3]);

    let supported_power_zones: Vec<PowerZones> = dbus::get_aura_supported_power_zones(path)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(PowerZones::from)
        .collect();

    let current_effect = dbus::get_aura_effect(path).await.unwrap_or(AuraEffect {
        mode: 0,
        zone: 0,
        colour1: Colour { r: 166, g: 0, b: 0 },
        colour2: Colour::BLACK,
        speed: "Med".to_string(),
        direction: "Right".to_string(),
    });

    let brightness = dbus::get_aura_brightness(path).await.unwrap_or(2);

    let all_modes = dbus::get_aura_all_mode_data(path).await.unwrap_or_default();

    let power_state = dbus::get_aura_led_power(path)
        .await
        .unwrap_or(LaptopAuraPower { states: Vec::new() });

    out.emit(AuraDeviceCmd::InitData {
        supported_modes,
        supported_zones,
        supported_brightness_levels: supported_brightness,
        supported_power_zones,
        current_effect,
        brightness,
        all_modes,
        power_state,
    });
}
