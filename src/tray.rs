use gpui::App;
use image::ImageReader;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuItem},
};

#[cfg(not(target_os = "windows"))]
const ICON: &[u8] = include_bytes!("../resources/icons/png/64x64.png");

#[cfg(target_os = "windows")]
const ICON: &[u8] = include_bytes!("../resources/windows/icon.ico");

pub struct SystemTray {
    tray: TrayIcon,
}

impl SystemTray {
    pub fn new(cx: &mut App) -> Self {
        let image = ImageReader::new(std::io::Cursor::new(ICON))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap()
            .into_rgba8();

        let (width, height) = image.dimensions();
        let rgba = image.into_raw();

        let menu = Menu::new();
        let exit_item = MenuItem::new("退出", true, None);
        let _ = menu.append_items(&[&exit_item]);

        let tray = TrayIconBuilder::new()
            .with_tooltip("BLive")
            .with_icon(Icon::from_rgba(rgba, width, height).unwrap())
            .with_menu(Box::new(menu))
            .build()
            .unwrap();

        // 克隆 exit_item 的 ID 以避免 Send trait 问题
        let exit_item_id = exit_item.id().clone();

        cx.background_executor()
            .spawn(async move {
                while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                    if event.id() == &exit_item_id {
                        println!("退出");
                        break;
                    }
                }
            })
            .detach();

        Self { tray }
    }

    pub fn show(&self) {
        let _ = self.tray.set_visible(true);
    }

    pub fn hide(&self) {
        let _ = self.tray.set_visible(false);
    }
}
