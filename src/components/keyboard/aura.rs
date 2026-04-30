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
use gtk4::gdk::RGBA;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::services::config::AppConfig;
use crate::services::dbus::{self, AuraEffect, AuraModeNum, Colour};

/// State for the Aura RGB keyboard lighting component.
pub struct AuraModel {
    /// Whether the `asusd` Aura D-Bus interface is reachable.
    asusd_available: bool,
    /// Currently active lighting mode.
    current_mode: AuraModeNum,
    /// Current brightness level (0 = Off … 3 = High).
    current_brightness: u32,
    /// Currently active primary colour.
    current_colour: Colour,
    /// Modes reported as supported by the hardware.
    supported_modes: Vec<AuraModeNum>,
    /// Mode selection dropdown — stored for imperative updates from `update_cmd`.
    mode_combo: adw::ComboRow,
    /// Brightness dropdown — stored for imperative updates from `update_cmd`.
    brightness_combo: adw::ComboRow,
    /// ActionRow that wraps `colour_button`; stored so `#[watch]` can toggle sensitivity.
    colour_row: adw::ActionRow,
    /// Colour picker — stored for imperative RGBA updates from `update_cmd`.
    colour_button: gtk::ColorDialogButton,
}

/// Input messages for the Aura lighting component.
#[derive(Debug)]
pub enum AuraMsg {
    /// User changed the lighting mode to the combo item at this index.
    ChangeMode(u32),
    /// User changed the brightness to the combo item at this index (0–3).
    ChangeBrightness(u32),
    /// User picked a new colour from the colour dialog.
    ChangeColour(RGBA),
    /// Apply all Aura settings from a profile without persisting them.
    LoadProfile {
        mode: u32,
        brightness: u32,
        colour_r: u8,
        colour_g: u8,
        colour_b: u8,
    },
}

/// Async command results for the Aura lighting component.
#[derive(Debug)]
pub enum AuraCommandOutput {
    /// Result of the initial Aura availability check.
    AsusdChecked(bool),
    /// Initial state read from the daemon: supported modes, current effect, brightness.
    InitData {
        supported_modes: Vec<AuraModeNum>,
        current_effect: AuraEffect,
        brightness: u32,
    },
    /// Confirmation that `SetLedModeData` succeeded.
    EffectSet,
    /// Confirmation that `SetBrightness` succeeded; carries the applied value.
    BrightnessSet(u32),
    /// An error message to forward as a toast notification.
    Error(String),
}

#[relm4::component(pub)]
impl Component for AuraModel {
    type Init = ();
    type Input = AuraMsg;
    type Output = String;
    type CommandOutput = AuraCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("aura_group_title"),
            set_description: Some(&t!("aura_group_desc")),

            add = &gtk::Label {
                #[watch]
                set_visible: !model.asusd_available,
                set_label: &t!("aura_missing_warning"),
                add_css_class: "error",
                set_wrap: true,
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
            },

            add = &model.mode_combo.clone() -> adw::ComboRow {
                set_title: &t!("aura_mode_title"),
                #[watch]
                set_sensitive: model.asusd_available,
            },

            add = &model.brightness_combo.clone() -> adw::ComboRow {
                #[watch]
                set_sensitive: model.asusd_available,
            },

            add = &model.colour_row.clone() -> adw::ActionRow {
                #[watch]
                set_sensitive: model.asusd_available
                    && !model.current_mode.is_colour_irrelevant(),
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let config = AppConfig::load();
        let profile = config.active_profile();

        let saved_mode = AuraModeNum::from(profile.aura_mode);
        let saved_brightness = profile.aura_brightness.min(3);
        let saved_colour = Colour {
            r: profile.aura_colour_r,
            g: profile.aura_colour_g,
            b: profile.aura_colour_b,
        };

        // Mode combo: single placeholder until the daemon responds
        let mode_combo = adw::ComboRow::new();
        mode_combo.set_model(Some(&gtk::StringList::new(&[&t!(saved_mode.i18n_key())])));
        mode_combo.connect_selected_notify({
            let sender = sender.clone();
            move |row| sender.input(AuraMsg::ChangeMode(row.selected()))
        });

        // Brightness combo: static Off / Low / Med / High list
        let brightness_labels = [
            t!("aura_brightness_off").to_string(),
            t!("aura_brightness_low").to_string(),
            t!("aura_brightness_med").to_string(),
            t!("aura_brightness_high").to_string(),
        ];
        let brightness_refs: Vec<&str> = brightness_labels.iter().map(|s| s.as_str()).collect();
        let brightness_combo = adw::ComboRow::new();
        brightness_combo.set_title(&t!("aura_brightness_title"));
        brightness_combo.set_model(Some(&gtk::StringList::new(&brightness_refs)));
        brightness_combo.set_selected(saved_brightness);
        brightness_combo.connect_selected_notify({
            let sender = sender.clone();
            move |row| sender.input(AuraMsg::ChangeBrightness(row.selected()))
        });

        // Colour button
        let colour_button = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
        let initial_rgba = RGBA::new(
            saved_colour.r as f32 / 255.0,
            saved_colour.g as f32 / 255.0,
            saved_colour.b as f32 / 255.0,
            1.0,
        );
        colour_button.set_rgba(&initial_rgba);
        colour_button.set_valign(gtk::Align::Center);
        colour_button.connect_rgba_notify({
            let sender = sender.clone();
            move |btn| sender.input(AuraMsg::ChangeColour(btn.rgba()))
        });

        // ActionRow wrapping the colour button
        let colour_row = adw::ActionRow::new();
        colour_row.set_title(&t!("aura_colour_title"));
        colour_row.add_suffix(&colour_button);

        let model = AuraModel {
            asusd_available: false,
            current_mode: saved_mode,
            current_brightness: saved_brightness,
            current_colour: saved_colour,
            supported_modes: Vec::new(),
            mode_combo,
            brightness_combo,
            colour_row,
            colour_button,
        };

        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    let available = dbus::check_aura_available().await;
                    out.emit(AuraCommandOutput::AsusdChecked(available));
                    if !available {
                        return;
                    }

                    let raw_modes = match dbus::get_aura_supported_modes().await {
                        Ok(v) => v,
                        Err(e) => {
                            out.emit(AuraCommandOutput::Error(e));
                            return;
                        }
                    };
                    let supported_modes: Vec<AuraModeNum> =
                        raw_modes.into_iter().map(AuraModeNum::from).collect();

                    let current_effect = match dbus::get_aura_effect().await {
                        Ok(e) => e,
                        Err(e) => {
                            out.emit(AuraCommandOutput::Error(e));
                            return;
                        }
                    };
                    let brightness = match dbus::get_aura_brightness().await {
                        Ok(b) => b,
                        Err(e) => {
                            out.emit(AuraCommandOutput::Error(e));
                            return;
                        }
                    };
                    out.emit(AuraCommandOutput::InitData {
                        supported_modes,
                        current_effect,
                        brightness,
                    });
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: AuraMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            AuraMsg::ChangeMode(idx) => {
                let Some(&mode) = self.supported_modes.get(idx as usize) else {
                    return;
                };
                if mode == self.current_mode {
                    return;
                }
                self.current_mode = mode;
                AppConfig::update(|c| c.active_profile_mut().aura_mode = mode as u32);

                let effect = self.build_effect();
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus::set_aura_effect(effect).await {
                                Ok(()) => out.emit(AuraCommandOutput::EffectSet),
                                Err(e) => out.emit(AuraCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            AuraMsg::ChangeBrightness(idx) => {
                if idx == self.current_brightness {
                    return;
                }
                self.current_brightness = idx;
                AppConfig::update(|c| c.active_profile_mut().aura_brightness = idx);

                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus::set_aura_brightness(idx).await {
                                Ok(b) => out.emit(AuraCommandOutput::BrightnessSet(b)),
                                Err(e) => out.emit(AuraCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            AuraMsg::ChangeColour(rgba) => {
                let colour = Colour {
                    r: (rgba.red() * 255.0) as u8,
                    g: (rgba.green() * 255.0) as u8,
                    b: (rgba.blue() * 255.0) as u8,
                };
                if colour == self.current_colour {
                    return;
                }
                self.current_colour = colour;
                AppConfig::update(|c| {
                    let p = c.active_profile_mut();
                    p.aura_colour_r = colour.r;
                    p.aura_colour_g = colour.g;
                    p.aura_colour_b = colour.b;
                });

                let effect = self.build_effect();
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus::set_aura_effect(effect).await {
                                Ok(()) => out.emit(AuraCommandOutput::EffectSet),
                                Err(e) => out.emit(AuraCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
            AuraMsg::LoadProfile {
                mode,
                brightness,
                colour_r,
                colour_g,
                colour_b,
            } => {
                if !self.asusd_available {
                    return;
                }

                // Update model fields before touching widgets to prevent spurious
                // signal callbacks from seeing stale `current_*` values.
                self.current_mode = AuraModeNum::from(mode);
                self.current_brightness = brightness.min(3);
                self.current_colour = Colour {
                    r: colour_r,
                    g: colour_g,
                    b: colour_b,
                };

                // Imperatively sync UI to the new profile values
                if let Some(idx) = self
                    .supported_modes
                    .iter()
                    .position(|&m| m == self.current_mode)
                {
                    self.mode_combo.set_selected(idx as u32);
                }
                self.brightness_combo.set_selected(self.current_brightness);
                self.colour_button.set_rgba(&RGBA::new(
                    colour_r as f32 / 255.0,
                    colour_g as f32 / 255.0,
                    colour_b as f32 / 255.0,
                    1.0,
                ));

                let effect = self.build_effect();
                let brightness_val = self.current_brightness;
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            if let Err(e) = dbus::set_aura_effect(effect).await {
                                out.emit(AuraCommandOutput::Error(e));
                                return;
                            }
                            match dbus::set_aura_brightness(brightness_val).await {
                                Ok(b) => out.emit(AuraCommandOutput::BrightnessSet(b)),
                                Err(e) => out.emit(AuraCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: AuraCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AuraCommandOutput::AsusdChecked(available) => {
                self.asusd_available = available;
            }
            AuraCommandOutput::InitData {
                supported_modes,
                current_effect,
                brightness,
            } => {
                // Update model fields first so that the combo `set_selected` callback
                // (which fires ChangeMode) sees the correct `current_mode` and no-ops.
                self.current_mode = AuraModeNum::from(current_effect.mode);
                self.current_colour = current_effect.colour1;
                self.current_brightness = brightness.min(3);
                self.supported_modes = supported_modes;

                let translated: Vec<String> = self
                    .supported_modes
                    .iter()
                    .map(|m| t!(m.i18n_key()).to_string())
                    .collect();
                let refs: Vec<&str> = translated.iter().map(|s| s.as_str()).collect();
                self.mode_combo.set_model(Some(&gtk::StringList::new(&refs)));

                let selected_idx = self
                    .supported_modes
                    .iter()
                    .position(|&m| m == self.current_mode)
                    .unwrap_or(0) as u32;
                self.mode_combo.set_selected(selected_idx);

                self.brightness_combo.set_selected(self.current_brightness);

                self.colour_button.set_rgba(&RGBA::new(
                    self.current_colour.r as f32 / 255.0,
                    self.current_colour.g as f32 / 255.0,
                    self.current_colour.b as f32 / 255.0,
                    1.0,
                ));
            }
            AuraCommandOutput::EffectSet => {
                tracing::info!(
                    "{}",
                    t!("aura_mode_set", mode = t!(self.current_mode.i18n_key()).to_string())
                );
            }
            AuraCommandOutput::BrightnessSet(b) => {
                tracing::info!("{}", t!("aura_brightness_set", level = b.to_string()));
            }
            AuraCommandOutput::Error(e) => {
                let _ = sender.output(e);
            }
        }
    }
}

impl AuraModel {
    /// Constructs an [`AuraEffect`] from the current model state, using sensible
    /// defaults for speed and direction.
    fn build_effect(&self) -> AuraEffect {
        AuraEffect {
            mode: self.current_mode as u32,
            zone: 0, // AuraZone::None — applies to the full keyboard
            colour1: self.current_colour,
            colour2: Colour { r: 0, g: 0, b: 0 },
            speed: "Med".to_string(),
            direction: "Right".to_string(),
        }
    }
}
