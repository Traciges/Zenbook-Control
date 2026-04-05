use crate::app::AppMsg;
use gtk4::prelude::ApplicationExt;
use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, Tray};
use relm4::Sender;
use rust_i18n::t;

pub struct AsusTray {
    pub app_sender: Sender<AppMsg>,
}

impl Tray for AsusTray {
    fn id(&self) -> String {
        "AsusHub".into()
    }

    fn title(&self) -> String {
        "Asus Hub".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let png_bytes = include_bytes!("../assets/trayicon.png");
        let Ok(img) = image::load_from_memory(png_bytes) else {
            return vec![];
        };
        let img = img.into_rgba8();
        let (width, height) = img.dimensions();
        let data: Vec<u8> = img
            .pixels()
            .flat_map(|p| [p[3], p[0], p[1], p[2]])
            .collect();
        vec![Icon {
            width: width as i32,
            height: height as i32,
            data,
        }]
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let sender_show = self.app_sender.clone();
        vec![
            MenuItem::Standard(StandardItem {
                label: t!("tray_show").to_string(),
                activate: Box::new(move |_| {
                    sender_show.emit(AppMsg::ShowWindow);
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: t!("tray_exit").to_string(),
                activate: Box::new(|_| {
                    relm4::main_application().quit();
                }),
                ..Default::default()
            }),
        ]
    }
}
