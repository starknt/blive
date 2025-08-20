use crate::error::{AppError, AppResult};
use chrono::Local;
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

/// 全局日志管理器实例
static GLOBAL_LOGGER: LazyLock<RwLock<LoggerManager>> = LazyLock::new(|| {
    let logger = LoggerManager::new(if cfg!(debug_assertions) {
        Level::DEBUG
    } else {
        Level::INFO
    })
    .expect("无法创建全局日志管理器");
    RwLock::new(logger)
});

pub struct LoggerManager {
    log_level: Level,
    initialized: bool,
}

impl LoggerManager {
    /// 创建新的日志管理器
    pub fn new(log_level: Level) -> AppResult<Self> {
        Ok(Self {
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
    }

    /// 记录应用关闭日志
    pub fn log_app_shutdown(&self) {
        tracing::info!("应用关闭");
    }

    /// 记录录制开始
    pub fn log_recording_start(&self, room_id: u64, quality: &str, file_path: &str) {
        tracing::info!(
            "开始录制 - 房间: {}, 质量: {}, 文件: {}",
            room_id,
            quality,
            file_path
        );
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
pub fn log_recording_start(room_id: u64, quality: &str, file_path: &str) {
    if let Ok(logger) = GLOBAL_LOGGER.read() {
        logger.log_recording_start(room_id, quality, file_path);
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
