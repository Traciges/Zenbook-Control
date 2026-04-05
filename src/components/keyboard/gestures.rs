use gtk::gdk;
use gtk::glib;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;
use tokio::sync::watch;

use crate::services::config::AppConfig;
use crate::services::edge_gestures;

pub struct GesturenModel {
    active: bool,
    loop_tx: Option<watch::Sender<bool>>,
}

#[derive(Debug)]
pub enum GesturenMsg {
    ToggleGestures(bool),
}

const GESTURE_IMG: &[u8] = include_bytes!("../../../assets/img/gesture.png");

#[relm4::component(pub)]
impl Component for GesturenModel {
    type Init = ();
    type Input = GesturenMsg;
    type Output = String;
    type CommandOutput = ();

    view! {
        adw::PreferencesGroup {
            set_title: &t!("gestures_group_title"),
            set_description: Some(&t!("gestures_group_desc")),

            add = &gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 24,

                #[name = "gesture_image"]
                append = &gtk::Picture {
                    set_width_request: 300,
                    set_valign: gtk::Align::Start,
                },

                append = &gtk::ListBox {
                    set_hexpand: true,
                    set_valign: gtk::Align::Start,
                    add_css_class: "boxed-list",

                    append = &adw::SwitchRow {
                        set_title: &t!("gestures_toggle_title"),

                        #[watch]
                        set_active: model.active,

                        connect_active_notify[sender] => move |s| {
                            sender.input(GesturenMsg::ToggleGestures(s.is_active()));
                        },
                    },

                    append = &adw::ActionRow {
                        set_title: &t!("gestures_volume_title"),
                        set_subtitle: &t!("gestures_volume_subtitle"),
                    },

                    append = &adw::ActionRow {
                        set_title: &t!("gestures_brightness_title"),
                        set_subtitle: &t!("gestures_brightness_subtitle"),
                    },

                    append = &adw::ActionRow {
                        set_title: &t!("gestures_media_title"),
                        set_subtitle: &t!("gestures_media_subtitle"),
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
        let active = AppConfig::load().input_gesten_aktiv;
        let loop_tx = if active {
            Some(start_gesture_loop())
        } else {
            None
        };
        let model = GesturenModel { active, loop_tx };
        let widgets = view_output!();

        let bytes = glib::Bytes::from_static(GESTURE_IMG);
        if let Ok(texture) = gdk::Texture::from_bytes(&bytes) {
            widgets.gesture_image.set_paintable(Some(&texture));
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: GesturenMsg, _sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            GesturenMsg::ToggleGestures(active) => {
                if active == self.active {
                    return;
                }
                self.active = active;
                AppConfig::update(|c| c.input_gesten_aktiv = active);

                if active {
                    self.loop_tx = Some(start_gesture_loop());
                } else {
                    // Dropping the sender causes the loop to exit
                    self.loop_tx = None;
                }
            }
        }
    }
}

fn start_gesture_loop() -> watch::Sender<bool> {
    let (tx, rx) = watch::channel(true);
    tokio::spawn(edge_gestures::run_gesture_loop(rx));
    tx
}
