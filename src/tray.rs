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

use crate::app::AppMsg;
use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, Tray};
use relm4::Sender;
use rust_i18n::t;

pub struct AyuzTray {
    pub app_sender: Sender<AppMsg>,
}

impl Tray for AyuzTray {
    fn id(&self) -> String {
        "Ayuz".into()
    }

    fn title(&self) -> String {
        "Ayuz".into()
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

    fn activate(&mut self, _x: i32, _y: i32) {
        self.app_sender.emit(AppMsg::ShowWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let sender_show = self.app_sender.clone();
        let sender_quit = self.app_sender.clone();
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
                activate: Box::new(move |_| {
                    sender_quit.emit(AppMsg::QuitApp);
                }),
                ..Default::default()
            }),
        ]
    }
}
