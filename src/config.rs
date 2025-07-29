use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// 配置管理器
pub struct ConfigManager {
    config_path: PathBuf,
    config: AppConfig,
}

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 应用设置
    pub app: AppSettings,
    /// 录制设置
    pub recording: RecordingSettings,
    /// 网络设置
    pub network: NetworkSettings,
    /// 界面设置
    pub ui: UISettings,
}

/// 应用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// 应用名称
    pub name: String,
    /// 应用版本
    pub version: String,
    /// 是否自动启动
    pub auto_start: bool,
    /// 是否开机自启
    pub startup_enabled: bool,
}

/// 录制设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSettings {
    /// 默认录制目录
    pub output_dir: String,
    /// 默认录制格式
    pub format: String,
    /// 默认录制质量
    pub quality: String,
    /// 是否自动重试
    pub auto_retry: bool,
    /// 最大重试次数
    pub max_retries: u32,
    /// 录制超时时间（秒）
    pub timeout: u64,
}

/// 网络设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSettings {
    /// 请求超时时间（秒）
    pub request_timeout: u64,
    /// 连接超时时间（秒）
    pub connect_timeout: u64,
    /// 是否使用代理
    pub use_proxy: bool,
    /// 代理地址
    pub proxy_url: Option<String>,
    /// 用户代理
    pub user_agent: String,
}

/// 界面设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UISettings {
    /// 主题
    pub theme: String,
    /// 语言
    pub language: String,
    /// 窗口宽度
    pub window_width: u32,
    /// 窗口高度
    pub window_height: u32,
    /// 是否记住窗口位置
    pub remember_window_position: bool,
    /// 窗口X坐标
    pub window_x: Option<i32>,
    /// 窗口Y坐标
    pub window_y: Option<i32>,
}

impl ConfigManager {
    /// 创建新的配置管理器
    pub fn new(config_path: PathBuf) -> AppResult<Self> {
        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            Self::default_config()
        };

        Ok(Self {
            config_path,
            config,
        })
    }

    /// 加载配置
    pub fn load(&mut self) -> AppResult<()> {
        if self.config_path.exists() {
            self.config = Self::load_config(&self.config_path)?;
        }
        Ok(())
    }

    /// 保存配置
    pub fn save(&self) -> AppResult<()> {
        // 确保配置目录存在
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| AppError::FileSystemError(e.to_string()))?;
        }

        let config_json = serde_json::to_string_pretty(&self.config)
            .map_err(|e| AppError::ConfigError(e.to_string()))?;

        fs::write(&self.config_path, config_json)
            .map_err(|e| AppError::FileSystemError(e.to_string()))?;

        Ok(())
    }

    /// 获取配置
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// 获取可变配置
    pub fn config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    /// 重置为默认配置
    pub fn reset_to_default(&mut self) {
        self.config = Self::default_config();
    }

    /// 加载配置文件
    fn load_config(path: &Path) -> AppResult<AppConfig> {
        let content =
            fs::read_to_string(path).map_err(|e| AppError::FileSystemError(e.to_string()))?;

        serde_json::from_str(&content).map_err(|e| AppError::ConfigError(e.to_string()))
    }

    /// 创建默认配置
    fn default_config() -> AppConfig {
        AppConfig {
            app: AppSettings {
                name: "B站录播姬".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                auto_start: false,
                startup_enabled: false,
            },
            recording: RecordingSettings {
                output_dir: Self::default_output_dir(),
                format: "flv".to_string(),
                quality: "original".to_string(),
                auto_retry: true,
                max_retries: 3,
                timeout: 30,
            },
            network: NetworkSettings {
                request_timeout: 30,
                connect_timeout: 10,
                use_proxy: false,
                proxy_url: None,
                user_agent: "B站录播姬/1.0".to_string(),
            },
            ui: UISettings {
                theme: "dark".to_string(),
                language: "zh-CN".to_string(),
                window_width: 1200,
                window_height: 800,
                remember_window_position: true,
                window_x: None,
                window_y: None,
            },
        }
    }

    /// 获取默认输出目录
    fn default_output_dir() -> String {
        if let Some(home) = std::env::home_dir() {
            home.join("Movies")
                .join("blive")
                .to_string_lossy()
                .to_string()
        } else {
            "./recordings".to_string()
        }
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        let config_path = Self::default_config_path();
        Self::new(config_path).unwrap_or_else(|_| Self {
            config_path: Self::default_config_path(),
            config: Self::default_config(),
        })
    }
}

impl ConfigManager {
    /// 获取默认配置路径
    fn default_config_path() -> PathBuf {
        if cfg!(debug_assertions) {
            PathBuf::from("config.json")
        } else {
            let mut path = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
            path.push(".config");
            path.push("blive");
            path.push("config.json");
            path
        }
    }
}
