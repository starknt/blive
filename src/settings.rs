use crate::{
    components::{SettingsModal, SettingsModalEvent},
    state::AppState,
};
use directories::ProjectDirs;
use gpui::{prelude::*, *};
use gpui_component::{
    ContextModal, IconName, Sizable,
    button::{Button, ButtonVariants},
};
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path, sync::LazyLock};

pub const APP_NAME: &str = "blive";
pub const DISPLAY_NAME: &str = "B站录播姬";
pub const DEFAULT_RECORD_NAME: &str = "{up_name}_{room_id}_{datetime}";

static SETTINGS_FILE: LazyLock<String> = LazyLock::new(|| {
    if cfg!(debug_assertions) {
        "target/settings.json".to_string()
    } else if let Some(project_dirs) = ProjectDirs::from_path(APP_NAME.into()) {
        project_dirs
            .config_dir()
            .join("settings.json")
            .to_string_lossy()
            .to_string()
    } else if cfg!(target_os = "windows") {
        std::env::home_dir()
            .unwrap()
            .join(format!("AppData/Local/{APP_NAME}/settings.json"))
            .to_string_lossy()
            .to_string()
    } else {
        std::env::home_dir()
            .unwrap()
            .join(format!(".config/{APP_NAME}/settings.json"))
            .to_string_lossy()
            .to_string()
    }
});

static DEFAULT_RECORD_DIR: LazyLock<String> = LazyLock::new(|| {
    let default = std::env::home_dir()
        .unwrap()
        .join(format!("Movies/{APP_NAME}"))
        .to_string_lossy()
        .to_string();

    if let Some(user_dirs) = directories::UserDirs::new() {
        if let Some(movies_dir) = user_dirs.video_dir() {
            movies_dir.join(APP_NAME).to_string_lossy().to_string()
        } else {
            default
        }
    } else {
        default
    }
});

const DEFAULT_THEME: &str = "Catppuccin Mocha";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RecordQuality {
    // 杜比
    Dolby,
    // 4K
    UHD4K,
    // 原画
    Original,
    // 蓝光
    BlueRay,
    // 超清
    UltraHD,
    // 高清
    HD,
    // 流畅
    Smooth,
}

impl fmt::Display for RecordQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordQuality::Dolby => write!(f, "杜比"),
            RecordQuality::UHD4K => write!(f, "4K"),
            RecordQuality::Original => write!(f, "原画"),
            RecordQuality::BlueRay => write!(f, "蓝光"),
            RecordQuality::UltraHD => write!(f, "超清"),
            RecordQuality::HD => write!(f, "高清"),
            RecordQuality::Smooth => write!(f, "流畅"),
        }
    }
}

impl RecordQuality {
    pub fn to_quality(&self) -> u32 {
        match self {
            RecordQuality::Dolby => 30000,
            RecordQuality::UHD4K => 20000,
            RecordQuality::Original => 10000,
            RecordQuality::BlueRay => 400,
            RecordQuality::UltraHD => 250,
            RecordQuality::HD => 150,
            RecordQuality::Smooth => 80,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub theme_name: SharedString,
    /// 录制质量
    pub quality: RecordQuality,
    /// 录制格式
    pub format: String,
    pub record_dir: String,
    pub rooms: Vec<RoomSettings>,
}

impl GlobalSettings {
    pub fn load() -> Self {
        // 读取配置文件
        let settings_path = &SETTINGS_FILE;
        let settings_path: &str = settings_path.as_str();
        let path = Path::new(settings_path);
        if path.exists()
            && let Ok(file_content) = std::fs::read_to_string(path)
        {
            if let Ok(settings) = serde_json::from_str::<GlobalSettings>(&file_content) {
                return settings;
            }

            return GlobalSettings::default();
        }

        GlobalSettings::default()
    }

    pub fn save(&self) {
        let settings_path = &SETTINGS_FILE;
        let settings_path: &str = settings_path.as_str();
        let path = Path::new(&settings_path);
        std::fs::write(path, serde_json::to_string(self).unwrap()).unwrap();
    }
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            quality: RecordQuality::Original,
            format: "flv".to_string(),
            record_dir: DEFAULT_RECORD_DIR.to_owned(),
            theme_name: DEFAULT_THEME.into(),
            rooms: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSettings {
    /// 房间号
    pub room_id: u64,
    /// 录制质量
    pub quality: Option<RecordQuality>,
    /// 录制格式
    pub format: Option<String>,
    /// 录制名称 {up_name}_{room_id}_{datetime}
    pub record_name: String,
}

impl RoomSettings {
    pub fn new(room_id: u64) -> Self {
        Self {
            room_id,
            quality: None,
            format: None,
            record_name: DEFAULT_RECORD_NAME.to_string(),
        }
    }
}

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            room_id: 0,
            quality: None,
            format: None,
            record_name: DEFAULT_RECORD_NAME.to_string(),
        }
    }
}

pub struct AppSettings {
    focus_handle: FocusHandle,
    setting_modal: Entity<SettingsModal>,
    _subscriptions: Vec<Subscription>,
}

impl AppSettings {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let setting_modal = SettingsModal::view(window, cx);

        Self {
            focus_handle: cx.focus_handle(),
            _subscriptions: vec![cx.subscribe(&setting_modal, Self::on_setting_modal_event)],
            setting_modal,
        }
    }

    fn on_setting_modal_event(_: Entity<SettingsModal>, event: &SettingsModalEvent, cx: &mut App) {
        match event {
            SettingsModalEvent::SaveSettings(settings) => {
                AppState::global_mut(cx).settings = settings.clone();
                settings.save();
            }
        }
    }
    fn show_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let setting_modal = self.setting_modal.clone();
        window.open_modal(cx, move |modal, _window, _cx| {
            modal
                .title("全局设置")
                .overlay(true)
                .overlay_closable(false)
                .child(setting_modal.clone())
        });
    }
}

impl Focusable for AppSettings {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().track_focus(&self.focus_handle).child(
            Button::new("settings")
                .icon(IconName::Settings)
                .ghost()
                .small()
                .on_click(cx.listener(|this, _, window, cx| this.show_modal(window, cx))),
        )
    }
}
