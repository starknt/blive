use directories::ProjectDirs;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::logger::log_user_action;
use gpui::SharedString;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    ops::{Add, AddAssign},
    path::Path,
    sync::LazyLock,
};

pub const APP_NAME: &str = "blive";
pub const DISPLAY_NAME: &str = "BLive";
pub const DEFAULT_RECORD_NAME: &str = "{up_name}_{room_title}_{datetime}";
const DEFAULT_THEME: &str = "Catppuccin Mocha";
const DEFAULT_VERSION: SettingsVersion = SettingsVersion::V1;

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

/// 配置版本枚举
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum SettingsVersion {
    V0 = 0,
    #[num_enum(default)]
    V1 = 1,
}

impl Serialize for SettingsVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let v: u32 = (*self).into(); // From IntoPrimitive
        serializer.serialize_u32(v)
    }
}

impl<'de> Deserialize<'de> for SettingsVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = u32::deserialize(deserializer)?;
        SettingsVersion::try_from(v).map_err(serde::de::Error::custom)
    }
}

impl Add for SettingsVersion {
    type Output = SettingsVersion;

    fn add(self, rhs: Self) -> Self::Output {
        let result = (self as u32) + (rhs as u32);
        match result {
            0 => SettingsVersion::V0,
            1 => SettingsVersion::V1,
            _ => SettingsVersion::V1, // 默认返回最新版本
        }
    }
}

impl AddAssign for SettingsVersion {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

/// 版本化配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedSettings {
    /// 配置版本
    pub version: SettingsVersion,
    /// 配置数据
    pub data: GlobalSettings,
}

impl Default for VersionedSettings {
    fn default() -> Self {
        Self {
            version: SettingsVersion::V1,
            data: GlobalSettings::default(),
        }
    }
}

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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub rooms: Vec<RoomSettings>,
}

impl GlobalSettings {
    pub fn load() -> Self {
        log_user_action("加载应用设置", None);

        // 读取配置文件
        let settings_path = &*SETTINGS_FILE;
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
            // 尝试使用迁移器加载和迁移配置
            match SettingsMigrator::migrate(&file_content) {
                Ok(migrated_settings) => {
                    log_user_action(
                        "设置文件加载并迁移成功",
                        Some(&format!("路径: {settings_path}")),
                    );
                    migrated_settings
                }
                Err(e) => {
                    log_user_action(
                        "设置文件迁移失败，使用默认设置",
                        Some(&format!("错误: {e}, 路径: {settings_path}")),
                    );
                    GlobalSettings::default()
                }
            }
        } else {
            GlobalSettings::default()
        };

        if !path.exists() {
            log_user_action(
                "设置文件不存在，使用默认设置",
                Some(&format!("路径: {settings_path}")),
            );
        }

        if settings.theme_name.is_empty() {
            log_user_action("主题名称为空，使用默认主题", Some(DEFAULT_THEME));
            settings.theme_name = DEFAULT_THEME.into();
        }

        settings
    }

    pub fn save(&self) {
        log_user_action("保存应用设置", None);

        let settings_path = &*SETTINGS_FILE;
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

        // 使用迁移器保存带版本信息的配置
        match SettingsMigrator::save_with_version(self) {
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoomSettings {
    /// 房间号
    pub room_id: u64,
    /// 录制目录
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub record_dir: Option<String>,
    /// 策略
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub strategy: Option<Strategy>,
    /// 录制质量
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub quality: Option<Quality>,
    /// 录制格式
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub format: Option<VideoContainer>,
    /// 录制编码
    #[serde(skip_serializing_if = "Option::is_none", default)]
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

    pub fn merge_global(&mut self, global_settings: &GlobalSettings) -> Self {
        Self {
            room_id: self.room_id,
            strategy: Some(self.strategy.unwrap_or(global_settings.strategy)),
            quality: Some(self.quality.unwrap_or(global_settings.quality)),
            format: Some(self.format.unwrap_or(global_settings.format)),
            codec: Some(self.codec.unwrap_or(global_settings.codec)),
            record_name: self.record_name.clone(),
            record_dir: match self.record_dir.clone().unwrap_or_default().is_empty() {
                true => Some(global_settings.record_dir.clone()),
                false => self.record_dir.clone(),
            },
        }
    }
}

/// 配置迁移器
pub struct SettingsMigrator;

impl SettingsMigrator {
    pub fn migrate(content: &str) -> Result<GlobalSettings, Box<dyn std::error::Error>> {
        log_user_action("开始配置迁移", None);

        // 尝试解析为版本化配置
        match serde_json::from_str::<VersionedSettings>(content) {
            Ok(versioned_settings) => {
                log_user_action(
                    "检测到版本化配置",
                    Some(&format!("版本: {:?}", versioned_settings.version)),
                );

                return Self::migrate_from_versioned(versioned_settings);
            }
            Err(e) => {
                log_user_action("解析版本化配置失败", Some(&format!("错误: {e}")));
            }
        }

        // 尝试解析为旧版本配置（无版本信息）
        match serde_json::from_str::<GlobalSettings>(content) {
            Ok(legacy_settings) => {
                log_user_action("检测到旧版本配置，开始迁移", None);
                return Self::migrate_from_legacy(legacy_settings);
            }
            Err(e) => {
                log_user_action("解析旧版本配置失败", Some(&format!("错误: {e}")));
            }
        }

        // 如果都解析失败，返回错误
        Err("无法解析配置文件格式".into())
    }

    /// 从版本化配置迁移到最新版本
    fn migrate_from_versioned(
        versioned_settings: VersionedSettings,
    ) -> Result<GlobalSettings, Box<dyn std::error::Error>> {
        let current_version = DEFAULT_VERSION;
        let mut settings = versioned_settings.data;
        let mut from_version = versioned_settings.version;

        log_user_action(
            "开始版本迁移",
            Some(&format!(
                "从版本 {from_version:?} 迁移到版本 {current_version:?}"
            )),
        );

        // 执行迁移链
        while from_version < current_version {
            settings = Self::migrate_single_version(from_version, settings)?;
            from_version = match from_version {
                SettingsVersion::V0 => SettingsVersion::V1,
                _ => break, // 未知版本，停止迁移
            };

            log_user_action(
                "版本迁移完成",
                Some(&format!("已迁移到版本 {from_version:?}")),
            );
        }

        Ok(settings)
    }

    /// 从旧版本配置迁移（无版本信息）
    fn migrate_from_legacy(
        legacy_settings: GlobalSettings,
    ) -> Result<GlobalSettings, Box<dyn std::error::Error>> {
        log_user_action("从旧版本配置迁移", None);

        // 从版本0开始迁移
        let mut settings = legacy_settings;
        let mut from_version = SettingsVersion::V0;

        while from_version < DEFAULT_VERSION {
            settings = Self::migrate_single_version(from_version, settings)?;
            from_version = match from_version {
                SettingsVersion::V0 => SettingsVersion::V1,
                _ => break, // 未知版本，停止迁移
            };
        }

        Ok(settings)
    }

    /// 执行单个版本的迁移
    fn migrate_single_version(
        from_version: SettingsVersion,
        settings: GlobalSettings,
    ) -> Result<GlobalSettings, Box<dyn std::error::Error>> {
        match from_version {
            SettingsVersion::V0 => Self::migrate_v0_to_v1(settings),
            _ => Ok(settings), // 未知版本，直接返回
        }
    }

    /// 从版本0迁移到版本1
    fn migrate_v0_to_v1(
        settings: GlobalSettings,
    ) -> Result<GlobalSettings, Box<dyn std::error::Error>> {
        log_user_action("执行版本0到版本1的迁移", None);

        let mut migrated_settings = settings;

        // 版本1的迁移逻辑：
        // 1. 确保所有必需字段都有默认值
        // 2. 添加新字段的默认值
        // 3. 修复可能的数据不一致问题

        // 确保主题名称不为空
        if migrated_settings.theme_name.is_empty() {
            migrated_settings.theme_name = DEFAULT_THEME.into();
            log_user_action("迁移：设置默认主题", Some(DEFAULT_THEME));
        }

        // 确保录制目录不为空
        if migrated_settings.record_dir.is_empty() {
            migrated_settings.record_dir = DEFAULT_RECORD_DIR.to_owned();
            log_user_action(
                "迁移：设置默认录制目录",
                Some(&migrated_settings.record_dir),
            );
        }

        // 确保房间设置中的录制名称不为空
        for room in &mut migrated_settings.rooms {
            if room.record_name.is_empty() {
                room.record_name = DEFAULT_RECORD_NAME.to_string();
                log_user_action(
                    "迁移：设置房间默认录制名称",
                    Some(&format!("房间ID: {}", room.room_id)),
                );
            }
        }

        log_user_action("版本0到版本1迁移完成", None);
        Ok(migrated_settings)
    }

    /// 保存配置时添加版本信息
    pub fn save_with_version(
        settings: &GlobalSettings,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let versioned_settings = VersionedSettings {
            version: DEFAULT_VERSION,
            data: settings.clone(),
        };

        serde_json::to_string_pretty(&versioned_settings).map_err(|e| e.into())
    }

    /// 备份配置文件
    pub fn backup_settings_file() -> Result<String, Box<dyn std::error::Error>> {
        let settings_path = &*SETTINGS_FILE;
        let path = Path::new(settings_path);

        if !path.exists() {
            return Err("配置文件不存在，无需备份".into());
        }

        let backup_path = format!(
            "{}.backup.{}",
            settings_path,
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        let backup_path = Path::new(&backup_path);

        std::fs::copy(path, backup_path)?;

        log_user_action(
            "配置文件备份成功",
            Some(&format!("备份路径: {}", backup_path.display())),
        );

        Ok(backup_path.to_string_lossy().to_string())
    }

    /// 验证配置文件的完整性
    pub fn validate_settings(settings: &GlobalSettings) -> Result<(), Box<dyn std::error::Error>> {
        // 验证主题名称
        if settings.theme_name.is_empty() {
            return Err("主题名称不能为空".into());
        }

        // 验证录制目录
        if settings.record_dir.is_empty() {
            return Err("录制目录不能为空".into());
        }

        // 验证房间设置
        for room in &settings.rooms {
            if room.record_name.is_empty() {
                return Err(format!("房间 {} 的录制名称不能为空", room.room_id).into());
            }
        }

        Ok(())
    }

    /// 获取配置文件的版本信息
    pub fn get_settings_version(
        content: &str,
    ) -> Result<SettingsVersion, Box<dyn std::error::Error>> {
        // 尝试解析为版本化配置
        if let Ok(versioned_settings) = serde_json::from_str::<VersionedSettings>(content) {
            return Ok(versioned_settings.version);
        }

        // 尝试解析为旧版本配置（无版本信息）
        if let Ok(_legacy_settings) = serde_json::from_str::<GlobalSettings>(content) {
            return Ok(SettingsVersion::V0); // 旧版本配置视为版本0
        }

        Err("无法解析配置文件格式".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_v0_to_v1() {
        // 创建版本0的配置（无版本信息）
        let v0_settings = GlobalSettings {
            strategy: Strategy::LowCost,
            theme_name: "".into(), // 空主题名称
            quality: Quality::Original,
            format: VideoContainer::FMP4,
            codec: StreamCodec::HEVC,
            record_dir: "".to_string(), // 空录制目录
            rooms: vec![RoomSettings {
                room_id: 12345,
                ..Default::default()
            }],
        };

        // 序列化为JSON
        let v0_json = serde_json::to_string(&v0_settings).unwrap();

        // 执行迁移
        let migrated_settings = SettingsMigrator::migrate(&v0_json).unwrap();

        // 验证迁移结果
        assert_eq!(migrated_settings.theme_name, DEFAULT_THEME);
        assert_eq!(migrated_settings.record_dir, *DEFAULT_RECORD_DIR);
        assert_eq!(migrated_settings.rooms[0].record_name, DEFAULT_RECORD_NAME);
    }

    #[test]
    fn test_migrate_v1_to_v1() {
        // 创建版本1的配置
        let v1_settings = GlobalSettings {
            strategy: Strategy::PriorityConfig,
            theme_name: "Test Theme".into(),
            quality: Quality::BlueRay,
            format: VideoContainer::FLV,
            codec: StreamCodec::AVC,
            record_dir: "/test/path".to_string(),
            rooms: vec![RoomSettings {
                room_id: 67890,
                record_dir: None,
                strategy: None,
                quality: None,
                format: None,
                codec: None,
                record_name: "test_name".to_string(),
            }],
        };

        // 创建版本化配置
        let versioned_settings = VersionedSettings {
            version: SettingsVersion::V1,
            data: v1_settings,
        };

        // 序列化为JSON
        let v1_json = serde_json::to_string(&versioned_settings).unwrap();

        // 执行迁移
        let migrated_settings = SettingsMigrator::migrate(&v1_json).unwrap();

        // 验证迁移结果（应该保持不变）
        assert_eq!(migrated_settings.theme_name, "Test Theme");
        assert_eq!(migrated_settings.record_dir, "/test/path");
        assert_eq!(migrated_settings.rooms[0].record_name, "test_name");
    }

    #[test]
    fn test_save_with_version() {
        let settings = GlobalSettings::default();
        let versioned_json = SettingsMigrator::save_with_version(&settings).unwrap();

        // 解析版本化JSON
        let versioned_settings: VersionedSettings = serde_json::from_str(&versioned_json).unwrap();

        // 验证版本信息
        assert_eq!(versioned_settings.version, DEFAULT_VERSION);
    }

    #[test]
    fn test_get_settings_version() {
        // 测试版本0配置
        let v0_settings = GlobalSettings::default();
        let v0_json = serde_json::to_string(&v0_settings).unwrap();
        assert_eq!(
            SettingsMigrator::get_settings_version(&v0_json).unwrap(),
            SettingsVersion::V0
        );

        // 测试版本1配置
        let v1_settings = VersionedSettings {
            version: SettingsVersion::V1,
            data: GlobalSettings::default(),
        };
        let v1_json = serde_json::to_string(&v1_settings).unwrap();
        assert_eq!(
            SettingsMigrator::get_settings_version(&v1_json).unwrap(),
            SettingsVersion::V1
        );
    }

    #[test]
    fn test_validate_settings() {
        // 测试有效配置
        let valid_settings = GlobalSettings::default();
        assert!(SettingsMigrator::validate_settings(&valid_settings).is_ok());

        // 测试无效配置（空主题名称）
        let invalid_settings = GlobalSettings {
            theme_name: "".into(),
            ..Default::default()
        };
        assert!(SettingsMigrator::validate_settings(&invalid_settings).is_err());

        // 测试无效配置（空录制目录）
        let invalid_settings = GlobalSettings {
            record_dir: "".to_string(),
            ..Default::default()
        };
        assert!(SettingsMigrator::validate_settings(&invalid_settings).is_err());

        // 测试无效配置（空房间录制名称）
        let mut invalid_settings = GlobalSettings::default();
        invalid_settings.rooms.push(RoomSettings {
            room_id: 12345,
            record_dir: None,
            strategy: None,
            quality: None,
            format: None,
            codec: None,
            record_name: "".to_string(),
        });
        assert!(SettingsMigrator::validate_settings(&invalid_settings).is_err());
    }
}
