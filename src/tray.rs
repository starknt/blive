use tray_item::{IconSource, TrayItem};

pub enum TrayMessage {
    OpenWindow,
    Quit,
}

pub struct SystemTray {
    tray: TrayItem,
}

#[cfg(not(windows))]
const ICON: &[u8] = include_bytes!("../resources/mac/icon.png");

fn load_icon_rgba(icon: &[u8]) -> IconSource {
    IconSource::Data {
        width: 0,
        height: 0,
        data: icon.to_vec(),
    }
}

impl SystemTray {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        #[cfg(windows)]
        let icon = IconSource::Resource("IDI_ICON_TRAY");
        #[cfg(not(windows))]
        let icon = load_icon_rgba(ICON);

        let mut tray = TrayItem::new("BLive 录制", icon).unwrap();

        #[cfg(target_os = "macos")]
        tray.inner_mut().add_label("BLive 录制").unwrap();
        #[cfg(target_os = "windows")]
        tray.inner_mut().set_tooltip("BLive 录制").unwrap();

        Self { tray }
    }

    pub fn display(&mut self) {
        self.tray.inner_mut().display();
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
