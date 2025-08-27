use tray_item::{IconSource, TrayItem};

pub enum TrayMessage {
    OpenWindow,
    Quit,
}

pub struct SystemTray {
    tray: TrayItem,
}

#[cfg(target_os = "macos")]
const ICON: &[u8] = include_bytes!("../resources/mac/icon.png");

#[cfg(target_os = "linux")]
const ICON: &[u8] = include_bytes!("../resources/icons/png/32x32.png");

#[cfg(not(windows))]
fn load_icon_rgba(icon: &[u8]) -> IconSource {
    let decoder_red = png::Decoder::new(icon);
    let (info_red, mut reader_red) = decoder_red.read_info().unwrap();
    let mut buf_red = vec![0; info_red.buffer_size()];
    reader_red.next_frame(&mut buf_red).unwrap();

    IconSource::Data {
        data: buf_red,
        height: 32,
        width: 32,
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
        #[cfg(target_os = "macos")]
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
