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

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

use crate::services::config::AppConfig;
use crate::services::dbus_animatrix::{
    self, AnimatrixHardwareType, AnimatrixStatus, BuiltinAnimations, DbusAnimeFrame,
    detect_animatrix_hardware,
};

// Collapses the four `ChangeXxxAnim` update arms. Each lookup-table entry is
// `(idx, &'static str)`; `field` is the cached `String` on `AnimatrixModel`
// and `profile_field` is the matching key on the active profile.
macro_rules! change_anim {
    ($self:ident, $sender:ident, $idx:expr, $table:expr, $field:ident, $profile_field:ident) => {{
        let val = anim_value(&$table, $idx).to_string();
        if val == $self.$field {
            return;
        }
        $self.$field = val;
        AppConfig::update(|c| {
            c.active_profile_mut().$profile_field = $self.$field.clone()
        });
        let anims = $self.build_animations();
        $sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    match dbus_animatrix::set_animatrix_builtin_animations(anims).await {
                        Ok(_) => out.emit(AnimatrixCommandOutput::Applied),
                        Err(e) => out.emit(AnimatrixCommandOutput::Error(e)),
                    }
                })
                .drop_on_shutdown()
        });
    }};
}

// Collapses the three `ToggleOffWhenXxx` update arms. `$setter` is the
// `dbus_animatrix::set_animatrix_off_when_*` async function for that toggle.
macro_rules! apply_off_toggle {
    ($self:ident, $sender:ident, $v:expr, $field:ident, $profile_field:ident, $setter:path) => {{
        if $v == $self.$field {
            return;
        }
        $self.$field = $v;
        AppConfig::update(|c| c.active_profile_mut().$profile_field = $v);
        $sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    match $setter($v).await {
                        Ok(_) => out.emit(AnimatrixCommandOutput::Applied),
                        Err(e) => out.emit(AnimatrixCommandOutput::Error(e)),
                    }
                })
                .drop_on_shutdown()
        });
    }};
}

// ── GIF catalogue ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum HwFilter {
    /// Works on GA401 only (rog_anime 1.3.0 fixed-size buffer).
    Ga401Only,
    /// Works only on GU604 - hidden until a newer library version is used.
    Gu604Only,
}

struct GifEntry {
    name: &'static str,
    path: &'static str,
    hw_filter: HwFilter,
}

impl GifEntry {
    fn supports(&self, hw: AnimatrixHardwareType) -> bool {
        match self.hw_filter {
            HwFilter::Ga401Only => hw == AnimatrixHardwareType::GA401,
            HwFilter::Gu604Only => hw == AnimatrixHardwareType::GU604,
        }
    }
}

static GIF_CATALOGUE: &[GifEntry] = &[
    // ROG
    GifEntry { name: "ROG City",             path: "asus/rog/ROG city.gif",              hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "ROG Glitch",           path: "asus/rog/ROG glitch.gif",            hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "For Those Who Dare",   path: "asus/rog/For-those-who-dare.gif",    hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "For Those Who Dare 2", path: "asus/rog/For-those-who-dare_2.gif",  hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Fragment",             path: "asus/rog/Fragment.gif",              hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Infinite Triangle",    path: "asus/rog/Infinite-triangle.gif",     hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Kaleidoscope 1",       path: "asus/rog/Kaleidoscope1.gif",         hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Kaleidoscope 2",       path: "asus/rog/Kaleidoscope2.gif",         hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Sunset",               path: "asus/rog/Sunset.gif",               hw_filter: HwFilter::Ga401Only },
    // Gaming
    GifEntry { name: "Bird",                 path: "asus/gaming/Bird.gif",               hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Controller",           path: "asus/gaming/Controller.gif",         hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Fight",                path: "asus/gaming/Fight.gif",              hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "FPS",                  path: "asus/gaming/FPS.gif",               hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Keyboard",             path: "asus/gaming/Keyboard.gif",           hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "MOBA",                 path: "asus/gaming/MOBA.gif",              hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "UFO",                  path: "asus/gaming/UFO.gif",               hw_filter: HwFilter::Ga401Only },
    // Festive
    GifEntry { name: "Cupid",                path: "asus/festive/Cupid.gif",             hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Firework",             path: "asus/festive/Firework.gif",          hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Halloween",            path: "asus/festive/Halloween.gif",         hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Happy Holiday",        path: "asus/festive/Happy Holiday.gif",     hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Happy New Year",       path: "asus/festive/Happy new year.gif",    hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Lantern",              path: "asus/festive/Lantern.gif",           hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Love U Mom",           path: "asus/festive/Love u mom.gif",        hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Mother's Day",         path: "asus/festive/Mother's day.gif",      hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Valentine's Day",      path: "asus/festive/Valentine's Day.gif",   hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Year of the Ox",       path: "asus/festive/Year of the Ox.gif",    hw_filter: HwFilter::Ga401Only },
    // Music
    GifEntry { name: "Diamond",              path: "asus/music/Diamond.gif",             hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "DJ",                   path: "asus/music/DJ.gif",                 hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Music Player",         path: "asus/music/Music-player.gif",        hw_filter: HwFilter::Ga401Only },
    // Trend
    GifEntry { name: "Dog",                  path: "asus/trend/Dog.gif",                hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Hero",                 path: "asus/trend/Hero.gif",               hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Ski",                  path: "asus/trend/Ski.gif",                hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "The Scream",           path: "asus/trend/The scream.gif",          hw_filter: HwFilter::Ga401Only },
    GifEntry { name: "Wave",                 path: "asus/trend/Wave.gif",               hw_filter: HwFilter::Ga401Only },
    // GU604 (reserved for future library support)
    GifEntry { name: "Fighting Games 2022",  path: "gu604/Fighting Games 2022_GU604.gif",            hw_filter: HwFilter::Gu604Only },
    GifEntry { name: "Halloween II",         path: "gu604/Halloween II_GU604.gif",                   hw_filter: HwFilter::Gu604Only },
    GifEntry { name: "Infinite Triangle",    path: "gu604/Infinite triangle_GU604.gif",              hw_filter: HwFilter::Gu604Only },
    GifEntry { name: "Lunar New Year",       path: "gu604/Lunar new year dragon dance_GU604.gif",    hw_filter: HwFilter::Gu604Only },
    GifEntry { name: "OMNI",                 path: "gu604/OMNI_GU604.gif",                           hw_filter: HwFilter::Gu604Only },
    GifEntry { name: "Pinball",              path: "gu604/PInball_GU604.gif",                        hw_filter: HwFilter::Gu604Only },
    GifEntry { name: "ROG Glitch",           path: "gu604/ROG glitch_GU604.gif",                     hw_filter: HwFilter::Gu604Only },
    GifEntry { name: "Wizard",               path: "gu604/Wizard_GU604.gif",                         hw_filter: HwFilter::Gu604Only },
];

// ── Animation string ↔ index helpers ─────────────────────────────────────────

const BOOT_ANIMS: [(&str, &str); 2] = [
    ("GlitchConstruction", "animatrix_boot_anim_glitch_construction"),
    ("StaticEmergence", "animatrix_boot_anim_static_emergence"),
];
const AWAKE_ANIMS: [(&str, &str); 2] = [
    ("BinaryBannerScroll", "animatrix_awake_anim_binary_banner"),
    ("RogLogoGlitch", "animatrix_awake_anim_rog_glitch"),
];
const SLEEP_ANIMS: [(&str, &str); 2] = [
    ("BannerSwipe", "animatrix_sleep_anim_banner_swipe"),
    ("Starfield", "animatrix_sleep_anim_starfield"),
];
const SHUTDOWN_ANIMS: [(&str, &str); 2] = [
    ("GlitchOut", "animatrix_shutdown_anim_glitch_out"),
    ("SeeYa", "animatrix_shutdown_anim_see_ya"),
];

fn anim_index(table: &[(&str, &str)], name: &str) -> u32 {
    table
        .iter()
        .position(|(v, _)| *v == name)
        .unwrap_or(0) as u32
}

fn anim_value(table: &[(&'static str, &'static str)], idx: u32) -> &'static str {
    table.get(idx as usize).map(|(v, _)| *v).unwrap_or(table[0].0)
}

// ── Component ─────────────────────────────────────────────────────────────────

pub struct AnimatrixModel {
    hardware_type: AnimatrixHardwareType,
    status: AnimatrixStatus,
    current_enable_display: bool,
    current_brightness: u32,
    current_builtins_enabled: bool,
    current_boot_anim: String,
    current_awake_anim: String,
    current_sleep_anim: String,
    current_shutdown_anim: String,
    current_off_unplugged: bool,
    current_off_suspended: bool,
    current_off_lid_closed: bool,
    available_gifs: Vec<&'static GifEntry>,
    current_gif_idx: u32,
    is_playing: bool,
    play_btn_label: String,
    playing_title: String,
    playback_cancel: Option<tokio::sync::oneshot::Sender<()>>,
    // Imperative widget refs for profile loading
    brightness_combo: adw::ComboRow,
    boot_combo: adw::ComboRow,
    awake_combo: adw::ComboRow,
    sleep_combo: adw::ComboRow,
    shutdown_combo: adw::ComboRow,
    gif_combo: adw::ComboRow,
}

impl AnimatrixModel {
    fn build_animations(&self) -> BuiltinAnimations {
        BuiltinAnimations {
            boot: self.current_boot_anim.clone(),
            awake: self.current_awake_anim.clone(),
            sleep: self.current_sleep_anim.clone(),
            shutdown: self.current_shutdown_anim.clone(),
        }
    }

    fn controls_sensitive(&self) -> bool {
        self.status == AnimatrixStatus::Available
    }

    fn builtins_sensitive(&self) -> bool {
        self.controls_sensitive() && self.current_enable_display
    }

    fn anim_combos_sensitive(&self) -> bool {
        self.builtins_sensitive() && self.current_builtins_enabled
    }

    fn gallery_visible(&self) -> bool {
        !self.available_gifs.is_empty() && self.status == AnimatrixStatus::Available
    }

    fn update_playback_labels(&mut self) {
        if self.is_playing {
            let name = self
                .available_gifs
                .get(self.current_gif_idx as usize)
                .map(|e| e.name)
                .unwrap_or("");
            self.playing_title = t!("animatrix_playing_label", name = name).to_string();
            self.play_btn_label = t!("animatrix_stop_button").to_string();
        } else {
            self.playing_title = String::new();
            self.play_btn_label = t!("animatrix_play_button").to_string();
        }
    }
}

#[derive(Debug)]
pub enum AnimatrixMsg {
    ToggleEnable(bool),
    ChangeBrightness(u32),
    ToggleBuiltins(bool),
    ChangeBootAnim(u32),
    ChangeAwakeAnim(u32),
    ChangeSleepAnim(u32),
    ChangeShutdownAnim(u32),
    ToggleOffWhenUnplugged(bool),
    ToggleOffWhenSuspended(bool),
    ToggleOffWhenLidClosed(bool),
    SelectGif(u32),
    TogglePlayback,
    LoadProfile {
        enable_display: bool,
        brightness: u32,
        builtins_enabled: bool,
        boot_anim: String,
        awake_anim: String,
        sleep_anim: String,
        shutdown_anim: String,
        off_unplugged: bool,
        off_suspended: bool,
        off_lid_closed: bool,
    },
}

#[derive(Debug)]
pub enum AnimatrixCommandOutput {
    StatusChecked(AnimatrixStatus),
    InitData {
        enable_display: bool,
        brightness: u32,
        builtins_enabled: bool,
        animations: BuiltinAnimations,
        off_unplugged: bool,
        off_suspended: bool,
        off_lid_closed: bool,
    },
    Applied,
    PlaybackStopped,
    Error(String),
}

#[relm4::component(pub)]
impl Component for AnimatrixModel {
    type Init = ();
    type Input = AnimatrixMsg;
    type Output = String;
    type CommandOutput = AnimatrixCommandOutput;

    view! {
        adw::PreferencesGroup {
            set_title: &t!("animatrix_group_title"),
            set_description: Some(&t!("animatrix_group_desc")),

            // Hardware not present warning
            #[template]
            add = &crate::components::widgets::DaemonWarningLabel {
                set_label: &t!("animatrix_hardware_missing_warning"),
                #[watch]
                set_visible: model.hardware_type == AnimatrixHardwareType::Unsupported,
            },

            // Daemon missing warning (hardware present but asusd not running)
            add = &gtk::Label {
                set_label: &t!("animatrix_daemon_missing_warning"),
                set_wrap: true,
                add_css_class: "dim-label",
                set_xalign: 0.0,
                set_margin_top: 8,
                set_margin_start: 12,
                set_margin_end: 12,
                set_margin_bottom: 4,
                #[watch]
                set_visible: model.hardware_type != AnimatrixHardwareType::Unsupported
                    && model.status == AnimatrixStatus::DaemonNotRunning,
            },

            // ── Controls ──────────────────────────────────────────────────────
            add = &adw::SwitchRow {
                set_title: &t!("animatrix_enable_title"),
                set_subtitle: &t!("animatrix_enable_subtitle"),
                #[watch]
                set_active: model.current_enable_display,
                #[watch]
                set_sensitive: model.controls_sensitive(),
                connect_active_notify[sender] => move |row| {
                    sender.input(AnimatrixMsg::ToggleEnable(row.is_active()));
                },
            },

            add = &model.brightness_combo.clone() -> adw::ComboRow {
                set_title: &t!("animatrix_brightness_title"),
                #[watch]
                set_sensitive: model.controls_sensitive(),
            },

            add = &adw::SwitchRow {
                set_title: &t!("animatrix_builtins_title"),
                set_subtitle: &t!("animatrix_builtins_subtitle"),
                #[watch]
                set_active: model.current_builtins_enabled,
                #[watch]
                set_sensitive: model.builtins_sensitive(),
                connect_active_notify[sender] => move |row| {
                    sender.input(AnimatrixMsg::ToggleBuiltins(row.is_active()));
                },
            },

            add = &model.boot_combo.clone() -> adw::ComboRow {
                set_title: &t!("animatrix_boot_anim_title"),
                #[watch]
                set_sensitive: model.anim_combos_sensitive(),
            },

            add = &model.awake_combo.clone() -> adw::ComboRow {
                set_title: &t!("animatrix_awake_anim_title"),
                #[watch]
                set_sensitive: model.anim_combos_sensitive(),
            },

            add = &model.sleep_combo.clone() -> adw::ComboRow {
                set_title: &t!("animatrix_sleep_anim_title"),
                #[watch]
                set_sensitive: model.anim_combos_sensitive(),
            },

            add = &model.shutdown_combo.clone() -> adw::ComboRow {
                set_title: &t!("animatrix_shutdown_anim_title"),
                #[watch]
                set_sensitive: model.anim_combos_sensitive(),
            },

            add = &adw::SwitchRow {
                set_title: &t!("animatrix_off_unplugged_title"),
                set_subtitle: &t!("animatrix_off_unplugged_subtitle"),
                #[watch]
                set_active: model.current_off_unplugged,
                #[watch]
                set_sensitive: model.controls_sensitive(),
                connect_active_notify[sender] => move |row| {
                    sender.input(AnimatrixMsg::ToggleOffWhenUnplugged(row.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("animatrix_off_suspended_title"),
                set_subtitle: &t!("animatrix_off_suspended_subtitle"),
                #[watch]
                set_active: model.current_off_suspended,
                #[watch]
                set_sensitive: model.controls_sensitive(),
                connect_active_notify[sender] => move |row| {
                    sender.input(AnimatrixMsg::ToggleOffWhenSuspended(row.is_active()));
                },
            },

            add = &adw::SwitchRow {
                set_title: &t!("animatrix_off_lid_closed_title"),
                set_subtitle: &t!("animatrix_off_lid_closed_subtitle"),
                #[watch]
                set_active: model.current_off_lid_closed,
                #[watch]
                set_sensitive: model.controls_sensitive(),
                connect_active_notify[sender] => move |row| {
                    sender.input(AnimatrixMsg::ToggleOffWhenLidClosed(row.is_active()));
                },
            },

            // ── GIF Gallery ───────────────────────────────────────────────────
            add = &adw::ActionRow {
                set_title: &t!("animatrix_gallery_title"),
                set_subtitle: &t!("animatrix_gallery_desc"),
                #[watch]
                set_visible: model.gallery_visible(),
            },

            add = &model.gif_combo.clone() -> adw::ComboRow {
                set_title: "",
                #[watch]
                set_visible: model.gallery_visible(),
                #[watch]
                set_sensitive: !model.is_playing,
            },

            add = &adw::ActionRow {
                #[watch]
                set_visible: model.gallery_visible(),
                #[watch]
                set_title: &model.playing_title,

                add_suffix = &gtk::Button {
                    #[watch]
                    set_label: &model.play_btn_label,
                    set_valign: gtk::Align::Center,
                    add_css_class: "suggested-action",
                    connect_clicked[sender] => move |_| {
                        sender.input(AnimatrixMsg::TogglePlayback);
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
        let hardware_type = detect_animatrix_hardware();

        let profile = AppConfig::load();
        let p = profile.active_profile();

        let available_gifs: Vec<&'static GifEntry> = GIF_CATALOGUE
            .iter()
            .filter(|e| e.supports(hardware_type))
            .collect();

        let br_off = t!("aura_brightness_off").to_string();
        let br_low = t!("aura_brightness_low").to_string();
        let br_med = t!("aura_brightness_med").to_string();
        let br_high = t!("aura_brightness_high").to_string();
        let brightness_refs: Vec<&str> = vec![
            br_off.as_str(), br_low.as_str(), br_med.as_str(), br_high.as_str(),
        ];
        let brightness_combo = adw::ComboRow::new();
        brightness_combo.set_model(Some(&gtk::StringList::new(&brightness_refs)));
        brightness_combo.set_selected(p.animatrix_brightness.min(3));
        brightness_combo.connect_selected_notify({
            let sender = sender.clone();
            move |row| sender.input(AnimatrixMsg::ChangeBrightness(row.selected()))
        });

        // Animation combos
        let boot_combo =
            build_anim_combo(&BOOT_ANIMS, &p.animatrix_boot_anim, {
                let sender = sender.clone();
                move |idx| sender.input(AnimatrixMsg::ChangeBootAnim(idx))
            });
        let awake_combo =
            build_anim_combo(&AWAKE_ANIMS, &p.animatrix_awake_anim, {
                let sender = sender.clone();
                move |idx| sender.input(AnimatrixMsg::ChangeAwakeAnim(idx))
            });
        let sleep_combo =
            build_anim_combo(&SLEEP_ANIMS, &p.animatrix_sleep_anim, {
                let sender = sender.clone();
                move |idx| sender.input(AnimatrixMsg::ChangeSleepAnim(idx))
            });
        let shutdown_combo =
            build_anim_combo(&SHUTDOWN_ANIMS, &p.animatrix_shutdown_anim, {
                let sender = sender.clone();
                move |idx| sender.input(AnimatrixMsg::ChangeShutdownAnim(idx))
            });

        // GIF combo
        let gif_combo = adw::ComboRow::new();
        if !available_gifs.is_empty() {
            let gif_names: Vec<&str> = available_gifs.iter().map(|e| e.name).collect();
            gif_combo.set_model(Some(&gtk::StringList::new(&gif_names)));
            gif_combo.connect_selected_notify({
                let sender = sender.clone();
                move |row| sender.input(AnimatrixMsg::SelectGif(row.selected()))
            });
        }

        let model = AnimatrixModel {
            hardware_type,
            status: AnimatrixStatus::DaemonNotRunning,
            current_enable_display: p.animatrix_enable_display,
            current_brightness: p.animatrix_brightness.min(3),
            current_builtins_enabled: p.animatrix_builtins_enabled,
            current_boot_anim: p.animatrix_boot_anim.clone(),
            current_awake_anim: p.animatrix_awake_anim.clone(),
            current_sleep_anim: p.animatrix_sleep_anim.clone(),
            current_shutdown_anim: p.animatrix_shutdown_anim.clone(),
            current_off_unplugged: p.animatrix_off_when_unplugged,
            current_off_suspended: p.animatrix_off_when_suspended,
            current_off_lid_closed: p.animatrix_off_when_lid_closed,
            available_gifs,
            current_gif_idx: 0,
            is_playing: false,
            play_btn_label: t!("animatrix_play_button").to_string(),
            playing_title: String::new(),
            playback_cancel: None,
            brightness_combo,
            boot_combo,
            awake_combo,
            sleep_combo,
            shutdown_combo,
            gif_combo,
        };

        let widgets = view_output!();

        if hardware_type != AnimatrixHardwareType::Unsupported {
            sender.command(move |out, shutdown| {
                shutdown
                    .register(async move {
                        let status = dbus_animatrix::check_animatrix_status().await;
                        out.emit(AnimatrixCommandOutput::StatusChecked(status));
                        if status != AnimatrixStatus::Available {
                            return;
                        }
                        let (enable, brightness, builtins, animations, off_plug, off_susp, off_lid) =
                            tokio::join!(
                                dbus_animatrix::get_animatrix_enable_display(),
                                dbus_animatrix::get_animatrix_brightness(),
                                dbus_animatrix::get_animatrix_builtins_enabled(),
                                dbus_animatrix::get_animatrix_builtin_animations(),
                                dbus_animatrix::get_animatrix_off_when_unplugged(),
                                dbus_animatrix::get_animatrix_off_when_suspended(),
                                dbus_animatrix::get_animatrix_off_when_lid_closed(),
                            );
                        if let (Ok(ed), Ok(br), Ok(bl), Ok(an), Ok(op), Ok(os), Ok(ol)) =
                            (enable, brightness, builtins, animations, off_plug, off_susp, off_lid)
                        {
                            out.emit(AnimatrixCommandOutput::InitData {
                                enable_display: ed,
                                brightness: br.min(3),
                                builtins_enabled: bl,
                                animations: an,
                                off_unplugged: op,
                                off_suspended: os,
                                off_lid_closed: ol,
                            });
                        }
                    })
                    .drop_on_shutdown()
            });
        }

        ComponentParts { model, widgets }
    }

    fn update(
        &mut self,
        msg: AnimatrixMsg,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AnimatrixMsg::ToggleEnable(v) => {
                if v == self.current_enable_display {
                    return;
                }
                self.current_enable_display = v;
                AppConfig::update(|c| c.active_profile_mut().animatrix_enable_display = v);
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus_animatrix::set_animatrix_enable_display(v).await {
                                Ok(_) => out.emit(AnimatrixCommandOutput::Applied),
                                Err(e) => out.emit(AnimatrixCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }

            AnimatrixMsg::ChangeBrightness(idx) => {
                if idx == self.current_brightness {
                    return;
                }
                self.current_brightness = idx;
                AppConfig::update(|c| c.active_profile_mut().animatrix_brightness = idx);
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus_animatrix::set_animatrix_brightness(idx).await {
                                Ok(_) => out.emit(AnimatrixCommandOutput::Applied),
                                Err(e) => out.emit(AnimatrixCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }

            AnimatrixMsg::ToggleBuiltins(v) => {
                if v == self.current_builtins_enabled {
                    return;
                }
                self.current_builtins_enabled = v;
                AppConfig::update(|c| c.active_profile_mut().animatrix_builtins_enabled = v);
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            match dbus_animatrix::set_animatrix_builtins_enabled(v).await {
                                Ok(_) => out.emit(AnimatrixCommandOutput::Applied),
                                Err(e) => out.emit(AnimatrixCommandOutput::Error(e)),
                            }
                        })
                        .drop_on_shutdown()
                });
            }

            AnimatrixMsg::ChangeBootAnim(idx) => {
                change_anim!(self, sender, idx, BOOT_ANIMS, current_boot_anim, animatrix_boot_anim);
            }
            AnimatrixMsg::ChangeAwakeAnim(idx) => {
                change_anim!(self, sender, idx, AWAKE_ANIMS, current_awake_anim, animatrix_awake_anim);
            }
            AnimatrixMsg::ChangeSleepAnim(idx) => {
                change_anim!(self, sender, idx, SLEEP_ANIMS, current_sleep_anim, animatrix_sleep_anim);
            }
            AnimatrixMsg::ChangeShutdownAnim(idx) => {
                change_anim!(self, sender, idx, SHUTDOWN_ANIMS, current_shutdown_anim, animatrix_shutdown_anim);
            }

            AnimatrixMsg::ToggleOffWhenUnplugged(v) => {
                apply_off_toggle!(self, sender, v, current_off_unplugged, animatrix_off_when_unplugged, dbus_animatrix::set_animatrix_off_when_unplugged);
            }
            AnimatrixMsg::ToggleOffWhenSuspended(v) => {
                apply_off_toggle!(self, sender, v, current_off_suspended, animatrix_off_when_suspended, dbus_animatrix::set_animatrix_off_when_suspended);
            }
            AnimatrixMsg::ToggleOffWhenLidClosed(v) => {
                apply_off_toggle!(self, sender, v, current_off_lid_closed, animatrix_off_when_lid_closed, dbus_animatrix::set_animatrix_off_when_lid_closed);
            }

            AnimatrixMsg::SelectGif(idx) => {
                self.current_gif_idx = idx;
            }

            AnimatrixMsg::TogglePlayback => {
                if self.is_playing {
                    if let Some(cancel) = self.playback_cancel.take() {
                        let _ = cancel.send(());
                    }
                    self.is_playing = false;
                    self.update_playback_labels();
                } else {
                    let Some(entry) = self.available_gifs.get(self.current_gif_idx as usize)
                    else {
                        return;
                    };
                    let gif_path = crate::sys_paths::anime_assets_dir().join(entry.path);
                    let hw_str = self.hardware_type.as_dbus_str().to_string();
                    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
                    self.playback_cancel = Some(cancel_tx);
                    self.is_playing = true;
                    self.update_playback_labels();

                    sender.command(move |out, shutdown| {
                        shutdown
                            .register(async move {
                                // Decode GIF frames in a blocking thread (disk I/O + pixel work)
                                let frames = tokio::task::spawn_blocking(move || {
                                    rog_anime::AnimeGif::from_diagonal_gif(
                                        &gif_path,
                                        rog_anime::AnimTime::Infinite,
                                        1.0,
                                    )
                                })
                                .await;

                                let frames = match frames {
                                    Ok(Ok(f)) => f,
                                    _ => {
                                        out.emit(AnimatrixCommandOutput::PlaybackStopped);
                                        return;
                                    }
                                };

                                if dbus_animatrix::animatrix_run_main_loop(false)
                                    .await
                                    .is_err()
                                {
                                    out.emit(AnimatrixCommandOutput::PlaybackStopped);
                                    return;
                                }

                                let mut cancel_rx = cancel_rx;
                                'outer: loop {
                                    for frame in frames.frames() {
                                        if cancel_rx.try_recv().is_ok() {
                                            break 'outer;
                                        }
                                        let dbus_frame = DbusAnimeFrame {
                                            data: frame.frame().get().to_vec(),
                                            anime_type: hw_str.clone(),
                                        };
                                        if dbus_animatrix::animatrix_write_frame(dbus_frame)
                                            .await
                                            .is_err()
                                        {
                                            break 'outer;
                                        }
                                        tokio::time::sleep(frame.delay()).await;
                                    }
                                }

                                let _ = dbus_animatrix::animatrix_run_main_loop(true).await;
                                out.emit(AnimatrixCommandOutput::PlaybackStopped);
                            })
                            .drop_on_shutdown()
                    });
                }
            }

            AnimatrixMsg::LoadProfile {
                enable_display,
                brightness,
                builtins_enabled,
                boot_anim,
                awake_anim,
                sleep_anim,
                shutdown_anim,
                off_unplugged,
                off_suspended,
                off_lid_closed,
            } => {
                if self.status != AnimatrixStatus::Available
                    || self.hardware_type == AnimatrixHardwareType::Unsupported
                {
                    return;
                }
                if let Some(cancel) = self.playback_cancel.take() {
                    let _ = cancel.send(());
                }
                self.is_playing = false;

                // Update model fields before syncing widgets (re-entrancy guard)
                self.current_enable_display = enable_display;
                self.current_brightness = brightness.min(3);
                self.current_builtins_enabled = builtins_enabled;
                self.current_boot_anim = boot_anim.clone();
                self.current_awake_anim = awake_anim.clone();
                self.current_sleep_anim = sleep_anim.clone();
                self.current_shutdown_anim = shutdown_anim.clone();
                self.current_off_unplugged = off_unplugged;
                self.current_off_suspended = off_suspended;
                self.current_off_lid_closed = off_lid_closed;

                self.brightness_combo.set_selected(self.current_brightness);
                self.boot_combo.set_selected(anim_index(&BOOT_ANIMS, &boot_anim));
                self.awake_combo.set_selected(anim_index(&AWAKE_ANIMS, &awake_anim));
                self.sleep_combo.set_selected(anim_index(&SLEEP_ANIMS, &sleep_anim));
                self.shutdown_combo.set_selected(anim_index(&SHUTDOWN_ANIMS, &shutdown_anim));

                let anims = self.build_animations();
                sender.command(move |out, shutdown| {
                    shutdown
                        .register(async move {
                            let _ = tokio::join!(
                                dbus_animatrix::set_animatrix_enable_display(enable_display),
                                dbus_animatrix::set_animatrix_brightness(brightness.min(3)),
                                dbus_animatrix::set_animatrix_builtins_enabled(builtins_enabled),
                                dbus_animatrix::set_animatrix_builtin_animations(anims),
                                dbus_animatrix::set_animatrix_off_when_unplugged(off_unplugged),
                                dbus_animatrix::set_animatrix_off_when_suspended(off_suspended),
                                dbus_animatrix::set_animatrix_off_when_lid_closed(off_lid_closed),
                            );
                            out.emit(AnimatrixCommandOutput::Applied);
                        })
                        .drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: AnimatrixCommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AnimatrixCommandOutput::StatusChecked(status) => {
                self.status = status;
            }

            AnimatrixCommandOutput::InitData {
                enable_display,
                brightness,
                builtins_enabled,
                animations,
                off_unplugged,
                off_suspended,
                off_lid_closed,
            } => {
                // Update model first, then widgets
                self.current_enable_display = enable_display;
                self.current_brightness = brightness;
                self.current_builtins_enabled = builtins_enabled;
                self.current_boot_anim = animations.boot.clone();
                self.current_awake_anim = animations.awake.clone();
                self.current_sleep_anim = animations.sleep.clone();
                self.current_shutdown_anim = animations.shutdown.clone();
                self.current_off_unplugged = off_unplugged;
                self.current_off_suspended = off_suspended;
                self.current_off_lid_closed = off_lid_closed;

                self.brightness_combo.set_selected(brightness);
                self.boot_combo.set_selected(anim_index(&BOOT_ANIMS, &animations.boot));
                self.awake_combo.set_selected(anim_index(&AWAKE_ANIMS, &animations.awake));
                self.sleep_combo.set_selected(anim_index(&SLEEP_ANIMS, &animations.sleep));
                self.shutdown_combo.set_selected(anim_index(&SHUTDOWN_ANIMS, &animations.shutdown));
            }

            AnimatrixCommandOutput::PlaybackStopped => {
                self.is_playing = false;
                self.playback_cancel = None;
                self.update_playback_labels();
            }

            AnimatrixCommandOutput::Applied => {}

            AnimatrixCommandOutput::Error(e) => {
                let _ = sender.output(e);
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_anim_combo(
    table: &[(&'static str, &'static str)],
    current: &str,
    on_select: impl Fn(u32) + 'static,
) -> adw::ComboRow {
    let labels: Vec<String> = table.iter().map(|(_, k)| t!(*k).to_string()).collect();
    let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let combo = adw::ComboRow::new();
    combo.set_model(Some(&gtk::StringList::new(&label_refs)));
    combo.set_selected(anim_index(table, current));
    combo.connect_selected_notify(move |row| {
        on_select(row.selected());
    });
    combo
}
