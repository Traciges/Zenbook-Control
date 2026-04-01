use crate::components::display::FarbskalaModel;
use crate::components::display::OledCareModel;
use crate::components::keyboard::AutoBeleuchtungModel;
use crate::components::keyboard::FnKeyModel;
use crate::components::keyboard::GesturenModel;
use crate::components::keyboard::RuhezustandModel;
use crate::components::keyboard::TouchpadModel;
use crate::components::system::battery::BatteryModel;
use crate::components::system::fan::FanModel;
use crate::tray;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use rust_i18n::t;

#[derive(Debug)]
pub enum AppMsg {
    ShowWindow,
    Fehler(String),
    SpracheSetzen(String),
}

pub struct AppModel {
    window: gtk4::glib::WeakRef<adw::ApplicationWindow>,
    toast_overlay: adw::ToastOverlay,
    _tray: ksni::Handle<tray::ZenbookTray>,
    battery: Controller<BatteryModel>,
    fan: Controller<FanModel>,
    oled_care: Controller<OledCareModel>,
    farbskala: Controller<FarbskalaModel>,
    fn_key: Controller<FnKeyModel>,
    gesten: Controller<GesturenModel>,
    touchpad: Controller<TouchpadModel>,
    auto_beleuchtung: Controller<AutoBeleuchtungModel>,
    ruhezustand: Controller<RuhezustandModel>,
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some(&t!("app_title")),
            set_default_size: (1200, 800),

            #[wrap(Some)]
            set_content = &model.toast_overlay.clone() -> adw::ToastOverlay {
                #[wrap(Some)]
                set_child = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        #[wrap(Some)]
                        set_title_widget = &adw::ViewSwitcher {
                            set_stack: Some(&my_stack),
                            set_policy: adw::ViewSwitcherPolicy::Wide,
                        },
                    },
                    set_content: Some(&my_stack),
                },
            }
        }
    }

    fn update(&mut self, message: AppMsg, _sender: ComponentSender<Self>) {
        match message {
            AppMsg::ShowWindow => {
                if let Some(window) = self.window.upgrade() {
                    window.set_visible(true);
                    window.present();
                }
            }
            AppMsg::Fehler(text) => {
                eprintln!("{} {}", t!("error_prefix"), text);
                let toast = adw::Toast::new(&text);
                toast.set_timeout(5);
                self.toast_overlay.add_toast(toast);
            }
            AppMsg::SpracheSetzen(lang) => {
                crate::services::config::AppConfig::update(|c| {
                    c.language = lang.clone();
                });
                rust_i18n::set_locale(&lang);
                let toast = adw::Toast::new(&t!("lang_restart_toast"));
                toast.set_timeout(5);
                self.toast_overlay.add_toast(toast);
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let fehler = |msg: String| AppMsg::Fehler(msg);
        let battery = BatteryModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);
        let fan = FanModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);
        let oled_care = OledCareModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);
        let farbskala = FarbskalaModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);
        let fn_key = FnKeyModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);
        let gesten = GesturenModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);
        let touchpad = TouchpadModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);
        let auto_beleuchtung = AutoBeleuchtungModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);
        let ruhezustand = RuhezustandModel::builder()
            .launch(())
            .forward(sender.input_sender(), fehler);

        let tray_svc = ksni::TrayService::new(tray::ZenbookTray {
            app_sender: sender.input_sender().clone(),
        });
        let tray_handle = tray_svc.handle();
        tray_svc.spawn();

        let toast_overlay = adw::ToastOverlay::new();

        let model = AppModel {
            window: root.downgrade(),
            toast_overlay,
            _tray: tray_handle,
            battery,
            fan,
            oled_care,
            farbskala,
            fn_key,
            gesten,
            touchpad,
            auto_beleuchtung,
            ruhezustand,
        };

        let battery_widget = model.battery.widget();
        let fan_widget = model.fan.widget();
        let oled_care_widget = model.oled_care.widget();
        let farbskala_widget = model.farbskala.widget();
        let fn_key_widget = model.fn_key.widget();
        let gesten_widget = model.gesten.widget();
        let touchpad_widget = model.touchpad.widget();
        let auto_beleuchtung_widget = model.auto_beleuchtung.widget();
        let ruhezustand_widget = model.ruhezustand.widget();

        let my_stack = adw::ViewStack::new();

        let anzeige_page = adw::PreferencesPage::new();
        anzeige_page.add(oled_care_widget);
        anzeige_page.add(farbskala_widget);
        my_stack.add_titled_with_icon(&anzeige_page, None, &t!("tab_display"), "monitor-symbolic");

        let tastatur_page = adw::PreferencesPage::new();
        tastatur_page.add(auto_beleuchtung_widget);
        tastatur_page.add(ruhezustand_widget);
        tastatur_page.add(fn_key_widget);
        tastatur_page.add(touchpad_widget);
        tastatur_page.add(gesten_widget);
        my_stack.add_titled_with_icon(
            &tastatur_page,
            None,
            &t!("tab_keyboard"),
            "input-keyboard-symbolic",
        );

        let system_page = adw::PreferencesPage::new();
        system_page.add(battery_widget);
        system_page.add(fan_widget);

        let lang_group = adw::PreferencesGroup::new();
        lang_group.set_title(&t!("app_settings_title"));

        let lang_row = adw::ActionRow::new();
        lang_row.set_title(&t!("language_title"));

        const SUPPORTED_LANGS: &[(&str, &str)] = &[("English", "en"), ("Deutsch", "de")];

        let display_names: Vec<&str> = SUPPORTED_LANGS.iter().map(|(name, _)| *name).collect();
        let lang_dropdown = gtk4::DropDown::from_strings(&display_names);
        lang_dropdown.set_valign(gtk4::Align::Center);

        let current_lang = crate::services::config::AppConfig::load().language;
        if let Some(idx) = SUPPORTED_LANGS
            .iter()
            .position(|(_, code)| *code == current_lang)
        {
            lang_dropdown.set_selected(idx as u32);
        }

        let sender_clone = sender.clone();
        lang_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if let Some(&(_, code)) = SUPPORTED_LANGS.get(idx) {
                sender_clone.input(AppMsg::SpracheSetzen(code.to_string()));
            }
        });

        lang_row.add_suffix(&lang_dropdown);
        lang_row.set_activatable_widget(Some(&lang_dropdown));
        lang_group.add(&lang_row);

        system_page.add(&lang_group);

        my_stack.add_titled_with_icon(
            &system_page,
            None,
            &t!("tab_system"),
            "preferences-system-symbolic",
        );

        let widgets = view_output!();

        root.connect_close_request(|window| {
            window.set_visible(false);
            gtk4::glib::Propagation::Stop
        });

        ComponentParts { model, widgets }
    }
}
