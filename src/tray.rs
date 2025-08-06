use tray_item::{IconSource, TrayItem};

pub enum TrayMessage {
    OpenWindow,
    Quit,
}

pub struct SystemTray {
    tray: TrayItem,
}

impl SystemTray {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        #[cfg(windows)]
        let icon = IconSource::Resource("IDI_ICON_TRAY");
        #[cfg(not(windows))]
        let icon = IconSource::Data("IDI_ICON_TRAY");

        let mut tray = TrayItem::new("Tray", icon).unwrap();

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
        self.tray.inner_mut().quit();
    }
}
