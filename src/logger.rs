use crate::error::{AppError, AppResult};
use crate::settings::APP_NAME;
use chrono::Local;
use std::fs;
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};
use tracing::Level;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::{FmtSubscriber, fmt::format::FmtSpan};

struct SystemTime;

impl FormatTime for SystemTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%Y-%m-%d %H:%M:%S"))
    }
}

/// 默认日志路径，使用LazyLock进行惰性初始化
static DEFAULT_LOG_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    if cfg!(debug_assertions) {
        PathBuf::from("logs/app.log")
    } else {
        let mut path = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".config");
        path.push(APP_NAME);
        path.push("logs");
        path.push("app.log");
        path
    }
});

/// 全局日志管理器实例
static GLOBAL_LOGGER: LazyLock<RwLock<LoggerManager>> = LazyLock::new(|| {
    let logger = LoggerManager::new(Level::INFO, Some(DEFAULT_LOG_PATH.clone()))
        .expect("无法创建全局日志管理器");
    RwLock::new(logger)
});

pub struct LoggerManager {
    log_file: Option<PathBuf>,
    log_level: Level,
    initialized: bool,
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
            initialized: false,
        })
    }

    /// 初始化日志系统
    pub fn init(&mut self) -> AppResult<()> {
        if self.initialized {
            return Ok(());
        }

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

        self.initialized = true;
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
            initialized: false,
        }
    }
}

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

/// 初始化全局日志系统
pub fn init_logger() -> AppResult<()> {
    let mut logger = GLOBAL_LOGGER
        .write()
        .map_err(|e| AppError::Unknown(format!("无法获取日志管理器写锁: {e}")))?;
    logger.init()
}

/// 设置日志级别
pub fn set_log_level(level: LogLevel) -> AppResult<()> {
    let mut logger = GLOBAL_LOGGER
        .write()
        .map_err(|e| AppError::Unknown(format!("无法获取日志管理器写锁: {e}")))?;
    logger.log_level = level.into();
    Ok(())
}

/// 获取默认日志路径
pub fn get_default_log_path() -> &'static PathBuf {
    &DEFAULT_LOG_PATH
}

// 全局日志记录函数，方便其他模块使用

/// 记录应用启动日志
pub fn log_app_start(version: &str) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_app_start(version);
    }
}

/// 记录应用关闭日志
pub fn log_app_shutdown() {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_app_shutdown();
    }
}

/// 记录录制开始
pub fn log_recording_start(room_id: u64, quality: &str) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_recording_start(room_id, quality);
    }
}

/// 记录录制停止
pub fn log_recording_stop(room_id: u64) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_recording_stop(room_id);
    }
}

/// 记录录制错误
pub fn log_recording_error(room_id: u64, error: &str) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_recording_error(room_id, error);
    }
}

/// 记录网络请求
pub fn log_network_request(url: &str, method: &str) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_network_request(url, method);
    }
}

/// 记录网络响应
pub fn log_network_response(status: u16, duration_ms: u64) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_network_response(status, duration_ms);
    }
}

/// 记录配置变更
pub fn log_config_change(key: &str, value: &str) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_config_change(key, value);
    }
}

/// 记录用户操作
pub fn log_user_action(action: &str, details: Option<&str>) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_user_action(action, details);
    }
}

/// 创建默认日志管理器（保持向后兼容性）
#[deprecated(since = "0.1.0", note = "请使用 init_logger() 函数代替")]
pub fn create_default_logger() -> AppResult<LoggerManager> {
    let log_level = Level::INFO;
    let log_file = DEFAULT_LOG_PATH.clone();

    LoggerManager::new(log_level, Some(log_file))
}
