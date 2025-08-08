use directories::ProjectDirs;

use crate::logger::log_user_action;
use gpui::SharedString;
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path, sync::LazyLock};

pub const APP_NAME: &str = "blive";
pub const DISPLAY_NAME: &str = "BLive";
pub const DEFAULT_RECORD_NAME: &str = "{up_name}_{room_title}_{datetime}";
const DEFAULT_THEME: &str = "Catppuccin Mocha";

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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, strum::EnumString)]
pub enum Strategy {
    // 优化CPU占用
    #[default]
    #[serde(rename = "低占用")]
    #[strum(serialize = "低占用")]
    LowCost,
    // 配置优先
    #[serde(rename = "配置优先")]
    #[strum(serialize = "配置优先")]
    PriorityConfig,
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Strategy::LowCost => write!(f, "低占用"),
            Strategy::PriorityConfig => write!(f, "配置优先"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, strum::EnumString)]
pub enum LiveProtocol {
    #[serde(rename = "http_stream")]
    #[strum(serialize = "http_stream")]
    HttpStream,
    #[default]
    #[serde(rename = "http_hls")]
    #[strum(serialize = "http_hls")]
    HttpHLS,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, strum::EnumString)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum VideoContainer {
    #[strum(serialize = "flv")]
    FLV,
    #[default]
    #[strum(serialize = "fmp4")]
    FMP4,
    #[strum(serialize = "ts")]
    TS,
}

impl VideoContainer {
    pub fn ext(&self) -> &str {
        match self {
            VideoContainer::FLV => "flv",
            VideoContainer::FMP4 => "mkv",
            VideoContainer::TS => "mkv",
        }
    }
}

impl fmt::Display for VideoContainer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VideoContainer::FLV => write!(f, "flv"),
            VideoContainer::FMP4 => write!(f, "fmp4"),
            VideoContainer::TS => write!(f, "ts"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, strum::EnumString)]
pub enum Quality {
    // 杜比
    #[serde(rename = "杜比")]
    #[strum(serialize = "杜比")]
    Dolby,
    // 4K
    #[serde(rename = "4K")]
    #[strum(serialize = "4K")]
    UHD4K,
    // 原画
    #[default]
    #[serde(rename = "原画")]
    #[strum(serialize = "原画")]
    Original,
    // 蓝光
    #[serde(rename = "蓝光")]
    #[strum(serialize = "蓝光")]
    BlueRay,
    // 超清
    #[serde(rename = "超清")]
    #[strum(serialize = "超清")]
    UltraHD,
    // 高清
    #[serde(rename = "高清")]
    #[strum(serialize = "高清")]
    HD,
    // 流畅
    #[serde(rename = "流畅")]
    #[strum(serialize = "流畅")]
    Smooth,
}

impl fmt::Display for Quality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Quality::Dolby => write!(f, "杜比"),
            Quality::UHD4K => write!(f, "4K"),
            Quality::Original => write!(f, "原画"),
            Quality::BlueRay => write!(f, "蓝光"),
            Quality::UltraHD => write!(f, "超清"),
            Quality::HD => write!(f, "高清"),
            Quality::Smooth => write!(f, "流畅"),
        }
    }
}

impl Quality {
    pub fn to_quality(&self) -> u32 {
        match self {
            Quality::Dolby => 30000,
            Quality::UHD4K => 20000,
            Quality::Original => 10000,
            Quality::BlueRay => 400,
            Quality::UltraHD => 250,
            Quality::HD => 150,
            Quality::Smooth => 80,
        }
    }
}

#[derive(Debug, Clone, Default, Copy, Deserialize, Serialize, PartialEq, strum::EnumString)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StreamCodec {
    #[strum(serialize = "avc")]
    AVC,
    #[default]
    #[strum(serialize = "hevc")]
    HEVC,
}

impl fmt::Display for StreamCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamCodec::AVC => write!(f, "avc"),
            StreamCodec::HEVC => write!(f, "hevc"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// 策略
    pub strategy: Strategy,
    /// 主题名称
    pub theme_name: SharedString,
    /// 录制质量
    pub quality: Quality,
    /// 录制格式
    pub format: VideoContainer,
    /// 录制编码
    pub codec: StreamCodec,
    /// 录制目录
    pub record_dir: String,
    /// 录制房间
    pub rooms: Vec<RoomSettings>,
}

impl GlobalSettings {
    pub fn load() -> Self {
        log_user_action("加载应用设置", None);

        // 读取配置文件
        let settings_path = &SETTINGS_FILE;
        let settings_path: &str = settings_path.as_str();
        let path = Path::new(settings_path);

        // ensure the settings directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                if std::fs::create_dir_all(parent).is_ok() {
                    log_user_action(
                        "设置目录创建成功",
                        Some(&format!("路径: {}", parent.display())),
                    );
                } else {
                    log_user_action(
                        "设置目录创建失败",
                        Some(&format!("路径: {}", parent.display())),
                    );
                };
            }
        };

        let mut settings = if path.exists()
            && let Ok(file_content) = std::fs::read_to_string(path)
        {
            if let Ok(settings) = serde_json::from_str::<GlobalSettings>(&file_content) {
                log_user_action("设置文件加载成功", Some(&format!("路径: {settings_path}")));

                return settings;
            }

            log_user_action(
                "设置文件解析失败，使用默认设置",
                Some(&format!("路径: {settings_path}")),
            );

            GlobalSettings::default()
        } else {
            GlobalSettings::default()
        };

        log_user_action(
            "设置文件不存在，使用默认设置",
            Some(&format!("路径: {settings_path}")),
        );

        if settings.theme_name.is_empty() {
            log_user_action("主题名称为空，使用默认主题", Some(DEFAULT_THEME));
            settings.theme_name = DEFAULT_THEME.into();
        }

        settings
    }

    pub fn save(&self) {
        log_user_action("保存应用设置", None);

        let settings_path = &SETTINGS_FILE;
        let settings_path: &str = settings_path.as_str();
        let path = Path::new(&settings_path);

        // ensure the settings directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                if std::fs::create_dir_all(parent).is_ok() {
                    log_user_action(
                        "设置目录创建成功",
                        Some(&format!("路径: {}", parent.display())),
                    );
                } else {
                    log_user_action(
                        "设置目录创建失败",
                        Some(&format!("路径: {}", parent.display())),
                    );
                };
            }
        };

        match serde_json::to_string_pretty(self) {
            Ok(json_str) => {
                if let Err(e) = std::fs::write(path, json_str) {
                    log_user_action("设置保存失败", Some(&format!("错误: {e}")));
                } else {
                    log_user_action("设置保存成功", Some(&format!("路径: {settings_path}")));
                }
            }
            Err(e) => {
                log_user_action("设置序列化失败", Some(&format!("错误: {e}")));
            }
        }
    }
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            strategy: Strategy::default(),
            quality: Quality::default(),
            format: VideoContainer::default(),
            codec: StreamCodec::default(),
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
    /// 录制目录
    pub record_dir: Option<String>,
    /// 策略
    pub strategy: Option<Strategy>,
    /// 录制质量
    pub quality: Option<Quality>,
    /// 录制格式
    pub format: Option<VideoContainer>,
    /// 录制编码
    pub codec: Option<StreamCodec>,
    /// 录制名称 {up_name}_{room_title}_{datetime}
    pub record_name: String,
}

impl RoomSettings {
    pub fn new(room_id: u64) -> Self {
        Self {
            room_id,
            record_dir: None,
            strategy: None,
            quality: None,
            format: None,
            codec: None,
            record_name: DEFAULT_RECORD_NAME.to_string(),
        }
    }
}
