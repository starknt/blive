use tray_item::{IconSource, TrayItem};

pub enum TrayMessage {
    OpenWindow,
    Quit,
}

pub struct SystemTray {
    tray: TrayItem,
}

#[cfg(not(windows))]
const ICON: &[u8] = include_bytes!("../resources/icons/png/64x64.png");

impl SystemTray {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        #[cfg(windows)]
        let icon = IconSource::Resource("IDI_ICON_TRAY");
        #[cfg(not(windows))]
        let icon = {
            use std::io::Cursor;

            let icon = image::ImageReader::new(Cursor::new(ICON))
                .with_guessed_format()
                .unwrap();

            let (width, height) = icon.into_dimensions().unwrap();

            IconSource::Data {
                width: width as i32,
                height: height as i32,
                data: ICON.to_vec(),
            }
        };

        let mut tray = TrayItem::new("BLive 录制", icon).unwrap();

        #[cfg(target_os = "macos")]
        tray.inner_mut().add_quit_item("退出");
        #[cfg(target_os = "windows")]
        tray.inner_mut().set_tooltip("BLive 录制").unwrap();

        Self { tray }
    }

    pub fn add_menu_item<F>(&mut self, label: &str, action: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.tray.add_menu_item(label, action).unwrap();
    }

    pub fn quit(&mut self) {
        #[cfg(windows)]
        self.tray.inner_mut().quit();
    }
}
