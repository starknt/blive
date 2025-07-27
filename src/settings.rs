use gpui::{prelude::*, *};
use gpui_component::{
    ContextModal, IconName, Sizable,
    button::{Button, ButtonVariants},
    v_flex,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[cfg(debug_assertions)]
const SETTINGS_FILE: &str = "target/settings.json";
#[cfg(not(debug_assertions))]
const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl RecordQuality {
    pub fn to_string(&self) -> &str {
        match self {
            RecordQuality::Dolby => "杜比",
            RecordQuality::UHD4K => "4K",
            RecordQuality::Original => "原画",
            RecordQuality::BlueRay => "蓝光",
            RecordQuality::UltraHD => "超清",
            RecordQuality::HD => "高清",
            RecordQuality::Smooth => "流畅",
        }
    }

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
    pub theme_name: Option<SharedString>,
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
        {
            // 1. 如果是 debug 模式，则从 target/settings.json 读取
            // 2. 如果是 release 模式，则从 settings.json 读取，在windows下，从 C:\Users\Administrator\AppData\Local\LiveRecoder\settings.json 读取，在mac下，从.config/LiveRecoder/settings.json 读取
            // 3. 如果文件不存在，则使用默认值
            #[cfg(debug_assertions)]
            {
                let path = std::path::Path::new(SETTINGS_FILE);
                if path.exists() {
                    serde_json::from_str(&std::fs::read_to_string(path).unwrap())
                        .unwrap_or_default()
                }
            }
            #[cfg(not(debug_assertions))]
            {
                let path = std::path::Path::new(SETTINGS_FILE);
                if path.exists() {
                    serde_json::from_str(std::fs::read_to_string(path).unwrap()).unwrap_or_default()
                }
            }

            GlobalSettings::default()
        }
    }

    pub fn save(&self) {
        let path = std::path::Path::new(SETTINGS_FILE);
        std::fs::write(path, serde_json::to_string(self).unwrap()).unwrap();
    }
}

impl Default for GlobalSettings {
    fn default() -> Self {
        let record_dir = {
            // 1. 如果是mac默认是 ~/Movies/LiveRecoder
            // 2. 如果是windows默认是 C:\Users\C\Movies\LiveRecoder
            std::env::home_dir().unwrap().join("Movies/LiveRecoder")
        };

        Self {
            quality: RecordQuality::Original,
            format: "flv".to_string(),
            record_dir: record_dir.to_string_lossy().to_string(),
            theme_name: None,
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
            record_name: "{up_name}_{room_id}_{datetime}".to_string(),
        }
    }
}

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            room_id: 0,
            quality: None,
            format: None,
            record_name: "{up_name}_{room_id}_{datetime}".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct AppSettings {
    #[allow(dead_code)]
    layout: Axis,
    focus_handle: FocusHandle,
}

impl AppSettings {
    pub fn new(cx: &mut App) -> Self {
        Self {
            layout: Axis::Vertical,
            focus_handle: cx.focus_handle(),
        }
    }

    fn show_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_modal(cx, move |modal, _window, _cx| {
            modal
                .title("设置")
                .content_center()
                .overlay(true)
                .overlay_closable(true)
                .child(
                    v_flex()
                        .gap_3()
                        .child("This is a modal dialog.")
                        .child("You can put anything here."),
                )
                .footer({
                    move |_, _, _, _| {
                        vec![
                            Button::new("open_dir")
                                .label("打开目录")
                                .on_click(move |_, _, cx| {
                                    cx.spawn(async move |cx| {
                                        if let Some(handle) =
                                            rfd::AsyncFileDialog::new().pick_folder().await
                                        {
                                            let _ =
                                                cx.update_global(move |state: &mut AppState, _| {
                                                    state.settings.record_dir = handle
                                                        .path()
                                                        .to_owned()
                                                        .to_string_lossy()
                                                        .to_string();
                                                    println!("{}", state.settings.record_dir);
                                                });
                                        }
                                    })
                                    .detach();
                                }),
                            Button::new("cancel")
                                .label("取消")
                                .on_click(move |_, window, cx| {
                                    window.close_modal(cx);
                                }),
                        ]
                    }
                })
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
