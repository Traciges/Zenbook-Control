use std::path::PathBuf;

use gtk4 as gtk;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use super::helpers::{apply_icm_profile, reset_icm_profile, setup_icm_profiles};
use crate::services::config::AppConfig;

fn filename_for_index(index: u32) -> Option<&'static str> {
    match index {
        1 => Some("ASUS_sRGB.icm"),
        2 => Some("ASUS_DCIP3.icm"),
        3 => Some("ASUS_DisplayP3.icm"),
        _ => None,
    }
}

pub struct FarbskalaModel {
    color_gamut_index: u32,
    icm_base_path: Option<PathBuf>,
}

impl FarbskalaModel {
    fn color_gamut_description(&self) -> std::borrow::Cow<'static, str> {
        match self.color_gamut_index {
            1 => t!("farbskala_desc_srgb"),
            2 => t!("farbskala_desc_dcip3"),
            3 => t!("farbskala_desc_displayp3"),
            _ => t!("farbskala_desc_native"),
        }
    }
}

#[derive(Debug)]
pub enum FarbskalaMsg {
    ChangeColorGamut(u32),
}

#[derive(Debug)]
pub enum FarbskalaCommandOutput {
    IcmReady(PathBuf),
    ProfileApplied(u32),
    Fehler(String),
}

#[relm4::component(pub)]
impl Component for FarbskalaModel {
    type Init = ();
    type Input = FarbskalaMsg;
    type Output = String;
    type CommandOutput = FarbskalaCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("farbskala_group_title"),
            set_description: Some(&t!("farbskala_group_desc")),

            add = &adw::ComboRow {
                set_title: &t!("farbskala_title"),
                #[watch]
                set_subtitle: &model.color_gamut_description(),
                set_model: Some(&farbskala_list),
                #[watch]
                set_selected: model.color_gamut_index,
                connect_selected_notify[sender] => move |row| {
                    sender.input(FarbskalaMsg::ChangeColorGamut(row.selected()));
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

        let native = t!("farbskala_option_native");
        let farbskala_list = gtk::StringList::new(&[&native, "sRGB", "DCI-P3", "Display P3"]);

        let model = FarbskalaModel {
            color_gamut_index: config.farbskala_index,
            icm_base_path: None,
        };

        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    match setup_icm_profiles().await {
                        Ok(path) => out.emit(FarbskalaCommandOutput::IcmReady(path)),
                        Err(e) => out.emit(FarbskalaCommandOutput::Fehler(e)),
                    }
                })
                .drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: FarbskalaMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            FarbskalaMsg::ChangeColorGamut(index) => {
                if index == self.color_gamut_index {
                    return;
                }
                self.color_gamut_index = index;
                AppConfig::update(|c| c.farbskala_index = index);

                if let Some(base) = self.icm_base_path.clone() {
                    apply_profile(index, base, &sender);
                } else {
                    tracing::warn!("{}", t!("farbskala_icm_path_not_ready"));
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: FarbskalaCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            FarbskalaCommandOutput::IcmReady(path) => {
                tracing::info!(
                    "{}",
                    t!("farbskala_icm_ready", path = path.display().to_string())
                );
                if self.color_gamut_index > 0 {
                    apply_profile(self.color_gamut_index, path.clone(), &sender);
                }
                self.icm_base_path = Some(path);
            }
            FarbskalaCommandOutput::ProfileApplied(index) => {
                tracing::info!(
                    "{}",
                    t!("farbskala_profile_applied", index = index.to_string())
                );
            }
            FarbskalaCommandOutput::Fehler(e) => {
                let _ = sender.output(e);
            }
        }
    }
}

fn apply_profile(index: u32, base: PathBuf, sender: &ComponentSender<FarbskalaModel>) {
    sender.command(move |out, shutdown| {
        shutdown
            .register(async move {
                let result = match filename_for_index(index) {
                    None => reset_icm_profile().await,
                    Some(filename) => apply_icm_profile(filename, &base).await,
                };
                match result {
                    Ok(()) => out.emit(FarbskalaCommandOutput::ProfileApplied(index)),
                    Err(e) => out.emit(FarbskalaCommandOutput::Fehler(e)),
                }
            })
            .drop_on_shutdown()
    });
}
