use crate::error::{AppError, AppResult};
use std::fs;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::{
    FmtSubscriber,
    fmt::{format::FmtSpan, time::SystemTime},
};

/// 日志管理器
pub struct LoggerManager {
    log_file: Option<PathBuf>,
    log_level: Level,
}

impl LoggerManager {
    /// 创建新的日志管理器
    pub fn new(log_level: Level, log_file: Option<PathBuf>) -> AppResult<Self> {
        // 确保日志目录存在
        if let Some(ref log_path) = log_file
            && let Some(parent) = log_path.parent()
        {
            fs::create_dir_all(parent).map_err(|e| AppError::FileSystemError(e.to_string()))?;
        }

        Ok(Self {
            log_file,
            log_level,
        })
    }

    /// 初始化日志系统
    pub fn init(&self) -> AppResult<()> {
        let builder = FmtSubscriber::builder()
            .with_timer(SystemTime)
            .with_level(true)
            .with_target(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_max_level(self.log_level);

        let subscriber = builder.finish();
        tracing::subscriber::set_global_default(subscriber)
            .map_err(|e| AppError::Unknown(format!("无法设置日志订阅者: {e}")))?;

        Ok(())
    }

    /// 记录应用启动日志
    pub fn log_app_start(&self, version: &str) {
        tracing::info!("应用启动 - 版本: {}", version);
        tracing::info!("日志级别: {:?}", self.log_level);
        if let Some(ref log_file) = self.log_file {
            tracing::info!("日志文件: {}", log_file.display());
        }
    }

    /// 记录应用关闭日志
    pub fn log_app_shutdown(&self) {
        tracing::info!("应用关闭");
    }

    /// 记录录制开始
    pub fn log_recording_start(&self, room_id: u64, quality: &str) {
        tracing::info!("开始录制 - 房间: {}, 质量: {}", room_id, quality);
    }

    /// 记录录制停止
    pub fn log_recording_stop(&self, room_id: u64) {
        tracing::info!("停止录制 - 房间: {}", room_id);
    }

    /// 记录录制错误
    pub fn log_recording_error(&self, room_id: u64, error: &str) {
        tracing::error!("录制错误 - 房间: {}, 错误: {}", room_id, error);
    }

    /// 记录网络请求
    pub fn log_network_request(&self, url: &str, method: &str) {
        tracing::debug!("网络请求 - {} {}", method, url);
    }

    /// 记录网络响应
    pub fn log_network_response(&self, status: u16, duration_ms: u64) {
        tracing::debug!("网络响应 - 状态: {}, 耗时: {}ms", status, duration_ms);
    }

    /// 记录配置变更
    pub fn log_config_change(&self, key: &str, value: &str) {
        tracing::info!("配置变更 - {}: {}", key, value);
    }

    /// 记录用户操作
    pub fn log_user_action(&self, action: &str, details: Option<&str>) {
        if let Some(details) = details {
            tracing::info!("用户操作 - {}: {}", action, details);
        } else {
            tracing::info!("用户操作 - {}", action);
        }
    }
}

impl Default for LoggerManager {
    fn default() -> Self {
        Self {
            log_file: None,
            log_level: Level::INFO,
        }
    }
}

/// 日志级别枚举
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Level::TRACE,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Info => Level::INFO,
            LogLevel::Warn => Level::WARN,
            LogLevel::Error => Level::ERROR,
        }
    }
}

/// 创建默认日志管理器
pub fn create_default_logger() -> AppResult<LoggerManager> {
    let log_level = Level::INFO;
    let log_file = get_default_log_path();

    LoggerManager::new(log_level, Some(log_file))
}

/// 获取默认日志路径
fn get_default_log_path() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from("logs/app.log")
    } else {
        let mut path = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".config");
        path.push("blive");
        path.push("logs");
        path.push("app.log");
        path
    }
}
