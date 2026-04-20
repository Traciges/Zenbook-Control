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

use std::path::PathBuf;

use gtk4 as gtk;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use super::helpers::{apply_icm_profile, reset_icm_profile, setup_icm_profiles};
use crate::services::{commands::is_kde_desktop, config::AppConfig};

fn filename_for_index(index: u32) -> Option<&'static str> {
    match index {
        1 => Some("Ayuz_sRGB.icm"),
        2 => Some("Ayuz_DCIP3.icm"),
        3 => Some("Ayuz_DisplayP3.icm"),
        _ => None,
    }
}

pub struct ColorGamutModel {
    color_gamut_index: u32,
    icm_base_path: Option<PathBuf>,
    kde_available: bool,
}

impl ColorGamutModel {
    fn color_gamut_description(&self) -> std::borrow::Cow<'static, str> {
        match self.color_gamut_index {
            1 => t!("color_gamut_desc_srgb"),
            2 => t!("color_gamut_desc_dcip3"),
            3 => t!("color_gamut_desc_displayp3"),
            _ => t!("color_gamut_desc_native"),
        }
    }
}

#[derive(Debug)]
pub enum ColorGamutMsg {
    ChangeColorGamut(u32),
    LoadProfile(u32),
}

#[derive(Debug)]
pub enum ColorGamutCommandOutput {
    IcmReady(PathBuf),
    ProfileApplied(u32),
    Error(String),
}

#[relm4::component(pub)]
impl Component for ColorGamutModel {
    type Init = ();
    type Input = ColorGamutMsg;
    type Output = String;
    type CommandOutput = ColorGamutCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("color_gamut_group_title"),
            set_description: Some(&t!("color_gamut_group_desc")),

            add = &gtk::Label {
                #[watch]
                set_visible: !model.kde_available,
                set_label: &t!("kde_required_warning"),
                add_css_class: "error",
                set_wrap: true,
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
            },

            add = &adw::ComboRow {
                set_title: &t!("color_gamut_title"),
                
                #[watch]
                set_sensitive: model.kde_available,
                
                #[watch]
                set_subtitle: &model.color_gamut_description(),
                set_model: Some(&gamut_list),
                #[watch]
                set_selected: model.color_gamut_index,
                connect_selected_notify[sender] => move |row| {
                    sender.input(ColorGamutMsg::ChangeColorGamut(row.selected()));
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

        let native = t!("color_gamut_option_native");
        let gamut_list = gtk::StringList::new(&[&native, "sRGB", "DCI-P3", "Display P3"]);

        let model = ColorGamutModel {
            color_gamut_index: config.active_profile().color_profile_index,
            icm_base_path: None,
            kde_available: is_kde_desktop(),
        };

        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    match setup_icm_profiles().await {
                        Ok(path) => out.emit(ColorGamutCommandOutput::IcmReady(path)),
                        Err(e) => out.emit(ColorGamutCommandOutput::Error(e)),
                    }
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ColorGamutMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            ColorGamutMsg::ChangeColorGamut(index) => {
                if index == self.color_gamut_index {
                    return;
                }
                self.color_gamut_index = index;
                AppConfig::update(|c| c.active_profile_mut().color_profile_index = index);

                if let Some(base) = self.icm_base_path.clone() {
                    apply_profile(index, base, &sender);
                } else {
                    tracing::warn!("{}", t!("color_gamut_icm_path_not_ready"));
                }
            }
            ColorGamutMsg::LoadProfile(index) => {
                if !self.kde_available {
                    return;
                }
                self.color_gamut_index = index;
                if let Some(base) = self.icm_base_path.clone() {
                    apply_profile(index, base, &sender);
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: ColorGamutCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            ColorGamutCommandOutput::IcmReady(path) => {
                tracing::info!(
                    "{}",
                    t!("color_gamut_icm_ready", path = path.display().to_string())
                );
                if self.color_gamut_index > 0 {
                    apply_profile(self.color_gamut_index, path.clone(), &sender);
                }
                self.icm_base_path = Some(path);
            }
            ColorGamutCommandOutput::ProfileApplied(index) => {
                tracing::info!(
                    "{}",
                    t!("color_gamut_profile_applied", index = index.to_string())
                );
            }
            ColorGamutCommandOutput::Error(e) => {
                let _ = sender.output(e);
            }
        }
    }
}

fn apply_profile(index: u32, base: PathBuf, sender: &ComponentSender<ColorGamutModel>) {
    sender.command(move |out, shutdown| {
        shutdown
            .register(async move {
                let result = match filename_for_index(index) {
                    None => reset_icm_profile().await,
                    Some(filename) => apply_icm_profile(filename, &base).await,
                };
                match result {
                    Ok(()) => out.emit(ColorGamutCommandOutput::ProfileApplied(index)),
                    Err(e) => out.emit(ColorGamutCommandOutput::Error(e)),
                }
            })
            .drop_on_shutdown()
    });
}
