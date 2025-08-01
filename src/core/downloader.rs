pub mod http_hls;
pub mod http_stream;
pub mod utils;

use crate::components::{RoomCard, RoomCardStatus};
use crate::core::downloader::{http_hls::HttpHlsDownloader, http_stream::HttpStreamDownloader};
use crate::core::http_client::HttpClient;
use crate::core::http_client::room::LiveRoomInfoData;
use crate::core::http_client::stream::{LiveRoomStreamUrl, PlayStream};
use crate::core::http_client::user::LiveUserInfo;
use crate::settings::{DEFAULT_RECORD_NAME, LiveProtocol, Quality, StreamCodec, VideoContainer};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
use gpui::{AsyncApp, WeakEntity};
use rand::Rng;
use std::sync::{Mutex, atomic};
use std::{borrow::Cow, collections::VecDeque, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct DownloaderContext {
    pub entity: WeakEntity<RoomCard>,
    pub client: HttpClient,
    pub room_id: u64,
    pub quality: Quality,
    pub format: VideoContainer,
    pub codec: StreamCodec,
    stats: Arc<Mutex<DownloadStats>>,
    is_running: Arc<atomic::AtomicBool>,
    event_queue: Arc<Mutex<VecDeque<DownloadEvent>>>,
}

impl DownloaderContext {
    pub fn new(
        entity: WeakEntity<RoomCard>,
        client: HttpClient,
        room_id: u64,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
    ) -> Self {
        Self {
            entity,
            client,
            room_id,
            quality,
            format,
            codec,
            stats: Arc::new(std::sync::Mutex::new(DownloadStats::default())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_queue: Arc::new(std::sync::Mutex::new(VecDeque::new())),
        }
    }

    pub fn update_card_status(&self, cx: &mut AsyncApp, status: RoomCardStatus) {
        if let Some(entity) = self.entity.upgrade() {
            let _ = entity.update(cx, |card, cx| {
                card.status = status;
                cx.notify();
            });
        }
    }

    /// 推送事件到队列
    pub fn push_event(&self, event: DownloadEvent) {
        if let Ok(mut queue) = self.event_queue.lock() {
            queue.push_back(event);
        }
    }

    /// 处理队列中的所有事件，返回处理的事件数量
    pub fn process_events(&self, cx: &mut AsyncApp) -> usize {
        let mut processed = 0;

        if let Ok(mut queue) = self.event_queue.lock() {
            while let Some(event) = queue.pop_front() {
                self.handle_event(cx, event);
                processed += 1;
            }
        }

        processed
    }

    /// 处理单个事件
    fn handle_event(&self, cx: &mut AsyncApp, event: DownloadEvent) {
        // 记录日志
        self.log_event(&event);

        // 更新UI状态并处理下载器状态
        match &event {
            DownloadEvent::Started { .. } => {
                self.update_card_status(cx, RoomCardStatus::Recording(0.0));
                // 确保运行状态为true
                self.set_running(true);
            }
            DownloadEvent::Progress {
                download_speed_kbps,
                ..
            } => {
                self.update_card_status(cx, RoomCardStatus::Recording(*download_speed_kbps));
                // 更新统计信息
                self.update_stats(|stats| {
                    stats.download_speed_kbps = *download_speed_kbps;
                });
            }
            DownloadEvent::Error { error } => {
                let status = if error.is_recoverable() {
                    RoomCardStatus::Error(format!("网络异常，正在重连: {error}"))
                } else {
                    RoomCardStatus::Error(format!("录制失败: {error}"))
                };
                self.update_card_status(cx, status);

                // 更新错误统计
                self.update_stats(|stats| {
                    stats.last_error = Some(error.to_string());
                });

                // 如果是不可恢复的错误，停止下载器
                if !error.is_recoverable() {
                    self.set_running(false);
                }
            }
            DownloadEvent::Reconnecting {
                attempt,
                delay_secs,
            } => {
                self.update_card_status(
                    cx,
                    RoomCardStatus::Error(format!(
                        "网络中断，第{attempt}次重连 ({delay_secs}秒后)"
                    )),
                );

                // 更新重连统计
                self.update_stats(|stats| {
                    stats.reconnect_count = *attempt;
                });

                // 重连期间保持运行状态
                self.set_running(true);
            }
            DownloadEvent::Completed { file_size, .. } => {
                self.update_card_status(cx, RoomCardStatus::Waiting);

                // 更新完成统计
                self.update_stats(|stats| {
                    stats.bytes_downloaded = *file_size;
                });

                // 下载完成，停止运行状态
                self.set_running(false);
            }
        }
    }

    /// 记录事件日志
    fn log_event(&self, event: &DownloadEvent) {
        match event {
            DownloadEvent::Started { file_path } => {
                #[cfg(debug_assertions)]
                eprintln!("🎬 开始录制到: {file_path}");
            }
            DownloadEvent::Progress {
                bytes_downloaded,
                download_speed_kbps,
                duration_ms,
            } => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "📊 下载进度: {:.2}MB, {:.1}kb/s, {}秒",
                    utils::pretty_bytes(*bytes_downloaded),
                    *download_speed_kbps,
                    duration_ms / 1000
                );
            }
            DownloadEvent::Error { error } => {
                if error.is_recoverable() {
                    eprintln!("⚠️  网络异常，正在重连: {error}");
                } else {
                    eprintln!("❌ 录制失败: {error}");
                }
            }
            DownloadEvent::Reconnecting {
                attempt,
                delay_secs,
            } => {
                eprintln!("🔄 网络中断，第{attempt}次重连 ({delay_secs}秒后)");
            }
            DownloadEvent::Completed {
                file_path,
                file_size,
            } => {
                let mb_size = *file_size as f64 / 1024.0 / 1024.0;
                eprintln!("✅ 录制完成: {file_path} ({mb_size:.2}MB)");
            }
        }
    }

    /// 启动事件处理任务
    pub fn start_event_processor(&self, cx: &mut AsyncApp) {
        let context = self.clone();

        cx.spawn(async move |cx| {
            while context.is_running() {
                // 每100ms处理一次事件队列
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;

                let processed = context.process_events(cx);

                // 如果没有事件处理且不在运行状态，退出循环
                if processed == 0 && !context.is_running() {
                    break;
                }
            }

            // 最后处理剩余的事件
            context.process_events(cx);
        })
        .detach();
    }

    /// 设置运行状态
    pub fn set_running(&self, running: bool) {
        self.is_running
            .store(running, std::sync::atomic::Ordering::Relaxed);
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 更新统计信息
    pub fn update_stats<F>(&self, updater: F)
    where
        F: FnOnce(&mut DownloadStats),
    {
        if let Ok(mut stats) = self.stats.lock() {
            updater(&mut stats);
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> DownloadStats {
        self.stats
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    /// 下载开始
    Started { file_path: String },
    /// 进度更新
    Progress {
        bytes_downloaded: u64,
        download_speed_kbps: f32,
        duration_ms: u64,
    },
    /// 下载完成
    Completed { file_path: String, file_size: u64 },
    /// 下载错误
    Error { error: DownloaderError },
    /// 网络重连中
    Reconnecting { attempt: u32, delay_secs: u64 },
}

// 下载统计信息
#[derive(Debug, Clone, Default)]
pub struct DownloadStats {
    pub bytes_downloaded: u64,
    pub download_speed_kbps: f32,
    pub duration_ms: u64,
    pub reconnect_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DownloaderError {
    // 网络相关错误
    #[error("网络连接失败: {message} (重试次数: {retry_count})")]
    NetworkConnectionFailed { message: String, retry_count: u32 },

    #[error("网络超时: {operation} 操作超时 ({timeout_secs}秒)")]
    NetworkTimeout {
        operation: String,
        timeout_secs: u64,
    },

    #[error("DNS解析失败: {host}")]
    DnsResolutionFailed { host: String },

    #[error("HTTP错误: {status_code} - {message}")]
    HttpError { status_code: u16, message: String },

    #[error("服务器拒绝连接: {url}")]
    ConnectionRefused { url: String },

    // 流媒体相关错误
    #[error("直播流已结束或不可用: {room_id}")]
    StreamUnavailable { room_id: u64 },

    #[error("流格式不支持: {format} (支持的格式: {supported_formats})")]
    UnsupportedStreamFormat {
        format: String,
        supported_formats: String,
    },

    #[error("流编码错误: {codec} - {details}")]
    StreamEncodingError { codec: String, details: String },

    #[error("流中断: {reason} (已下载: {bytes_downloaded} 字节)")]
    StreamInterrupted {
        reason: String,
        bytes_downloaded: u64,
    },

    // FFmpeg相关错误
    #[error("FFmpeg进程启动失败: {command} - {stderr}")]
    FfmpegStartupFailed { command: String, stderr: String },

    #[error("FFmpeg运行时错误: {error_type} - {message}")]
    FfmpegRuntimeError { error_type: String, message: String },

    #[error("FFmpeg编解码错误: {codec} - {details}")]
    FfmpegCodecError { codec: String, details: String },

    #[error("FFmpeg进程意外退出: 退出码 {exit_code}")]
    FfmpegProcessCrashed { exit_code: i32 },

    // 文件系统相关错误
    #[error("文件创建失败: {path} - {reason}")]
    FileCreationFailed { path: String, reason: String },

    #[error("文件写入失败: {path} - {bytes_written}/{total_bytes} 字节")]
    FileWriteFailed {
        path: String,
        bytes_written: u64,
        total_bytes: u64,
    },

    #[error("磁盘空间不足: 需要 {required_mb}MB，可用 {available_mb}MB")]
    InsufficientDiskSpace { required_mb: u64, available_mb: u64 },

    #[error("文件权限错误: {path} - {permission_type}")]
    FilePermissionError {
        path: String,
        permission_type: String,
    },

    #[error("目录创建失败: {path} - {reason}")]
    DirectoryCreationFailed { path: String, reason: String },

    // 配置相关错误
    #[error("无效的录制配置: {field} = {value} ({reason})")]
    InvalidRecordingConfig {
        field: String,
        value: String,
        reason: String,
    },

    #[error("不支持的视频质量: {quality} (房间 {room_id} 支持: {available_qualities})")]
    UnsupportedQuality {
        quality: String,
        room_id: u64,
        available_qualities: String,
    },

    #[error("不支持的视频编码: {codec} (支持: {supported_codecs})")]
    UnsupportedCodec {
        codec: String,
        supported_codecs: String,
    },

    #[error("输出路径无效: {path} - {reason}")]
    InvalidOutputPath { path: String, reason: String },

    // 认证和权限错误
    #[error("直播间访问被拒绝: 房间 {room_id} ({reason})")]
    RoomAccessDenied { room_id: u64, reason: String },

    #[error("用户认证失败: {user_id} - {reason}")]
    AuthenticationFailed { user_id: String, reason: String },

    #[error("权限不足: 需要 {required_permission} 权限")]
    InsufficientPermissions { required_permission: String },

    // 资源和限制错误
    #[error("内存不足: 需要 {required_mb}MB，可用 {available_mb}MB")]
    InsufficientMemory { required_mb: u64, available_mb: u64 },

    #[error("并发下载数量超限: {current_downloads}/{max_downloads}")]
    ConcurrencyLimitExceeded {
        current_downloads: u32,
        max_downloads: u32,
    },

    #[error("下载速度过慢: {current_kbps}kbps < {min_required_kbps}kbps")]
    DownloadTooSlow {
        current_kbps: f32,
        min_required_kbps: f32,
    },

    // 系统和环境错误
    #[error("系统资源不可用: {resource} - {reason}")]
    SystemResourceUnavailable { resource: String, reason: String },

    #[error("依赖程序缺失: {program} (版本要求: {required_version})")]
    MissingDependency {
        program: String,
        required_version: String,
    },

    #[error("操作系统不支持: {operation} 在 {os} 上不可用")]
    UnsupportedOperation { operation: String, os: String },

    // 通用错误（向后兼容）
    #[error("网络错误: {0}")]
    NetworkError(String),

    #[error("文件系统错误: {0}")]
    FileSystemError(String),

    #[error("FFmpeg错误: {0}")]
    FfmpegError(String),

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("流错误: {0}")]
    StreamError(String),
}

impl DownloaderError {
    /// 判断错误是否可恢复
    pub fn is_recoverable(&self) -> bool {
        match self {
            DownloaderError::NetworkConnectionFailed { .. } => true,
            DownloaderError::NetworkTimeout { .. } => true,
            DownloaderError::DnsResolutionFailed { .. } => true,
            DownloaderError::HttpError { status_code, .. } => {
                // 4xx 客户端错误通常是可恢复的
                (400..500).contains(status_code)
            }
            DownloaderError::ConnectionRefused { .. } => true,
            DownloaderError::StreamUnavailable { .. } => true,
            DownloaderError::UnsupportedStreamFormat { .. } => true,
            DownloaderError::StreamEncodingError { .. } => true,
            DownloaderError::StreamInterrupted { .. } => true,
            DownloaderError::FfmpegStartupFailed { .. } => true,
            DownloaderError::FfmpegRuntimeError { .. } => true,
            DownloaderError::FfmpegCodecError { .. } => true,
            DownloaderError::FfmpegProcessCrashed { exit_code } => {
                // 非零退出码通常表示严重错误
                *exit_code != 0
            }
            DownloaderError::FileCreationFailed { .. } => false,
            DownloaderError::FileWriteFailed { .. } => false,
            DownloaderError::InsufficientDiskSpace { .. } => false,
            DownloaderError::FilePermissionError { .. } => false,
            DownloaderError::DirectoryCreationFailed { .. } => false,
            DownloaderError::InvalidRecordingConfig { .. } => false,
            DownloaderError::UnsupportedQuality { .. } => false,
            DownloaderError::UnsupportedCodec { .. } => false,
            DownloaderError::InvalidOutputPath { .. } => false,
            DownloaderError::RoomAccessDenied { .. } => false,
            DownloaderError::AuthenticationFailed { .. } => false,
            DownloaderError::InsufficientPermissions { .. } => false,
            DownloaderError::InsufficientMemory { .. } => false,
            DownloaderError::ConcurrencyLimitExceeded { .. } => false,
            DownloaderError::DownloadTooSlow { .. } => false,
            DownloaderError::SystemResourceUnavailable { .. } => false,
            DownloaderError::MissingDependency { .. } => false,
            DownloaderError::UnsupportedOperation { .. } => false,
            DownloaderError::NetworkError(_) => true,
            DownloaderError::FileSystemError(_) => false,
            DownloaderError::FfmpegError(_) => false,
            DownloaderError::ConfigError(_) => false,
            DownloaderError::StreamError(_) => false,
        }
    }

    /// 获取错误的严重程度
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // 网络错误通常是临时的
            DownloaderError::NetworkConnectionFailed { .. }
            | DownloaderError::NetworkTimeout { .. }
            | DownloaderError::DnsResolutionFailed { .. }
            | DownloaderError::ConnectionRefused { .. }
            | DownloaderError::NetworkError(_) => ErrorSeverity::Warning,

            // HTTP错误根据状态码判断
            DownloaderError::HttpError { status_code, .. } => {
                if (500..600).contains(status_code) {
                    ErrorSeverity::Error // 服务器错误
                } else {
                    ErrorSeverity::Warning // 客户端错误
                }
            }

            // 流相关错误通常是警告
            DownloaderError::StreamUnavailable { .. }
            | DownloaderError::StreamInterrupted { .. }
            | DownloaderError::StreamError(_) => ErrorSeverity::Warning,

            // 格式和编码错误是配置问题
            DownloaderError::UnsupportedStreamFormat { .. }
            | DownloaderError::StreamEncodingError { .. }
            | DownloaderError::UnsupportedQuality { .. }
            | DownloaderError::UnsupportedCodec { .. } => ErrorSeverity::Error,

            // FFmpeg错误根据类型判断
            DownloaderError::FfmpegStartupFailed { .. }
            | DownloaderError::FfmpegProcessCrashed { .. } => ErrorSeverity::Critical,
            DownloaderError::FfmpegRuntimeError { .. }
            | DownloaderError::FfmpegCodecError { .. }
            | DownloaderError::FfmpegError(_) => ErrorSeverity::Error,

            // 文件系统错误通常是严重的
            DownloaderError::FileCreationFailed { .. }
            | DownloaderError::FileWriteFailed { .. }
            | DownloaderError::FilePermissionError { .. }
            | DownloaderError::DirectoryCreationFailed { .. }
            | DownloaderError::FileSystemError(_) => ErrorSeverity::Error,

            // 资源不足是关键错误
            DownloaderError::InsufficientDiskSpace { .. }
            | DownloaderError::InsufficientMemory { .. } => ErrorSeverity::Critical,

            // 配置错误
            DownloaderError::InvalidRecordingConfig { .. }
            | DownloaderError::InvalidOutputPath { .. }
            | DownloaderError::ConfigError(_) => ErrorSeverity::Error,

            // 权限和认证错误
            DownloaderError::RoomAccessDenied { .. }
            | DownloaderError::AuthenticationFailed { .. }
            | DownloaderError::InsufficientPermissions { .. } => ErrorSeverity::Error,

            // 系统限制和环境错误
            DownloaderError::ConcurrencyLimitExceeded { .. }
            | DownloaderError::DownloadTooSlow { .. }
            | DownloaderError::SystemResourceUnavailable { .. }
            | DownloaderError::MissingDependency { .. }
            | DownloaderError::UnsupportedOperation { .. } => ErrorSeverity::Critical,
        }
    }

    /// 获取错误分类
    pub fn category(&self) -> ErrorCategory {
        match self {
            DownloaderError::NetworkConnectionFailed { .. }
            | DownloaderError::NetworkTimeout { .. }
            | DownloaderError::DnsResolutionFailed { .. }
            | DownloaderError::HttpError { .. }
            | DownloaderError::ConnectionRefused { .. }
            | DownloaderError::NetworkError(_) => ErrorCategory::Network,

            DownloaderError::StreamUnavailable { .. }
            | DownloaderError::UnsupportedStreamFormat { .. }
            | DownloaderError::StreamEncodingError { .. }
            | DownloaderError::StreamInterrupted { .. }
            | DownloaderError::StreamError(_) => ErrorCategory::Stream,

            DownloaderError::FfmpegStartupFailed { .. }
            | DownloaderError::FfmpegRuntimeError { .. }
            | DownloaderError::FfmpegCodecError { .. }
            | DownloaderError::FfmpegProcessCrashed { .. }
            | DownloaderError::FfmpegError(_) => ErrorCategory::Ffmpeg,

            DownloaderError::FileCreationFailed { .. }
            | DownloaderError::FileWriteFailed { .. }
            | DownloaderError::InsufficientDiskSpace { .. }
            | DownloaderError::FilePermissionError { .. }
            | DownloaderError::DirectoryCreationFailed { .. }
            | DownloaderError::FileSystemError(_) => ErrorCategory::FileSystem,

            DownloaderError::InvalidRecordingConfig { .. }
            | DownloaderError::UnsupportedQuality { .. }
            | DownloaderError::UnsupportedCodec { .. }
            | DownloaderError::InvalidOutputPath { .. }
            | DownloaderError::ConfigError(_) => ErrorCategory::Configuration,

            DownloaderError::RoomAccessDenied { .. }
            | DownloaderError::AuthenticationFailed { .. }
            | DownloaderError::InsufficientPermissions { .. } => ErrorCategory::Authentication,

            DownloaderError::InsufficientMemory { .. }
            | DownloaderError::ConcurrencyLimitExceeded { .. }
            | DownloaderError::DownloadTooSlow { .. }
            | DownloaderError::SystemResourceUnavailable { .. }
            | DownloaderError::MissingDependency { .. }
            | DownloaderError::UnsupportedOperation { .. } => ErrorCategory::System,
        }
    }

    /// 便捷方法：创建网络连接失败错误
    pub fn network_connection_failed(message: impl Into<String>, retry_count: u32) -> Self {
        Self::NetworkConnectionFailed {
            message: message.into(),
            retry_count,
        }
    }

    /// 便捷方法：创建网络超时错误
    pub fn network_timeout(operation: impl Into<String>, timeout_secs: u64) -> Self {
        Self::NetworkTimeout {
            operation: operation.into(),
            timeout_secs,
        }
    }

    /// 便捷方法：创建HTTP错误
    pub fn http_error(status_code: u16, message: impl Into<String>) -> Self {
        Self::HttpError {
            status_code,
            message: message.into(),
        }
    }

    /// 便捷方法：创建流不可用错误
    pub fn stream_unavailable(room_id: u64) -> Self {
        Self::StreamUnavailable { room_id }
    }

    /// 便捷方法：创建FFmpeg启动失败错误
    pub fn ffmpeg_startup_failed(command: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self::FfmpegStartupFailed {
            command: command.into(),
            stderr: stderr.into(),
        }
    }

    /// 便捷方法：创建文件创建失败错误
    pub fn file_creation_failed(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::FileCreationFailed {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// 便捷方法：创建磁盘空间不足错误
    pub fn insufficient_disk_space(required_mb: u64, available_mb: u64) -> Self {
        Self::InsufficientDiskSpace {
            required_mb,
            available_mb,
        }
    }
}

/// 错误严重程度
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// 警告 - 可能影响功能但不会导致失败
    Warning,
    /// 错误 - 会导致操作失败但可以恢复
    Error,
    /// 关键 - 严重错误，需要立即处理
    Critical,
}

/// 错误分类
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    /// 网络相关错误
    Network,
    /// 流媒体相关错误
    Stream,
    /// FFmpeg相关错误
    Ffmpeg,
    /// 文件系统相关错误
    FileSystem,
    /// 配置相关错误
    Configuration,
    /// 认证和权限相关错误
    Authentication,
    /// 系统和环境相关错误
    System,
}

pub trait Downloader {
    /// 开始下载
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()>;

    /// 停止下载
    fn stop(&mut self) -> Result<()>;

    /// 获取下载状态
    fn status(&self) -> DownloadStatus;

    /// 获取下载统计信息
    fn stats(&self) -> DownloadStats;
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    /// 未开始
    NotStarted,
    /// 下载中
    Downloading,
    /// 已完成
    Completed,
    /// 重连中
    Reconnecting,
    /// 错误
    Error(String),
}

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// 输出路径
    pub output_path: String,
    /// 是否覆盖
    pub overwrite: bool,
    /// 超时时间（秒）
    pub timeout: u64,
    /// 重试次数
    pub retry_count: u32,
    /// 编码
    pub codec: StreamCodec,
    /// 视频容器
    pub format: VideoContainer,
    /// 画质
    pub quality: Quality,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            output_path: "download".to_string(),
            overwrite: false,
            timeout: 30,
            retry_count: 3,
            codec: StreamCodec::default(),
            format: VideoContainer::default(),
            quality: Quality::default(),
        }
    }
}

pub enum DownloaderType {
    HttpStream(HttpStreamDownloader),
    HttpHls(HttpHlsDownloader),
}

pub struct DownloaderFilenameTemplate {
    pub up_name: String,
    pub room_id: u64,
    pub room_title: String,
    pub room_description: String,
    pub room_area_name: String,
    pub date: String,
    pub datetime: String,
}

impl leon::Values for DownloaderFilenameTemplate {
    fn get_value(&self, key: &str) -> Option<Cow<'_, str>> {
        match key {
            "up_name" => Some(Cow::Borrowed(&self.up_name)),
            "room_id" => Some(Cow::Owned(self.room_id.to_string())),
            "datetime" => Some(Cow::Borrowed(&self.datetime)),
            "room_title" => Some(Cow::Borrowed(&self.room_title)),
            "room_description" => Some(Cow::Borrowed(&self.room_description)),
            "room_area_name" => Some(Cow::Borrowed(&self.room_area_name)),
            "date" => Some(Cow::Borrowed(&self.date)),
            _ => None,
        }
    }
}

pub struct BLiveDownloader {
    context: DownloaderContext,
    downloader: Option<DownloaderType>,
    // 网络重连相关字段
    max_reconnect_attempts: u32,
    reconnect_delay: Duration,
    is_auto_reconnect: bool,
}

impl BLiveDownloader {
    pub fn new(
        room_id: u64,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
        client: HttpClient,
        entity: WeakEntity<RoomCard>,
    ) -> Self {
        let context: DownloaderContext =
            DownloaderContext::new(entity, client, room_id, quality, format, codec);

        Self {
            context,
            downloader: None,
            max_reconnect_attempts: u32::MAX,        // 无限重试
            reconnect_delay: Duration::from_secs(1), // 初始延迟1秒
            is_auto_reconnect: true,                 // 是否启用自动重连
        }
    }

    fn update_card_status(&self, cx: &mut AsyncApp, status: RoomCardStatus) {
        self.context.update_card_status(cx, status);
    }

    /// 设置重连参数
    pub fn set_reconnect_config(
        &mut self,
        max_attempts: u32,
        initial_delay: Duration,
        auto_reconnect: bool,
    ) {
        self.max_reconnect_attempts = max_attempts;
        self.reconnect_delay = initial_delay;
        self.is_auto_reconnect = auto_reconnect;
    }

    /// 计算指数退避延迟，最大等待时间30分钟
    fn calculate_backoff_delay(&self, retry_count: u32) -> Duration {
        const MAX_DELAY: Duration = Duration::from_secs(30 * 60); // 30分钟

        // 指数退避：1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 1800(30分钟)
        let exponential_delay = self.reconnect_delay * (2_u32.pow(retry_count.min(10)));

        // 限制最大延迟为30分钟
        if exponential_delay > MAX_DELAY {
            MAX_DELAY
        } else {
            exponential_delay
        }
    }

    /// 获取直播流信息
    async fn get_stream_info(&self) -> Result<LiveRoomStreamUrl> {
        let mut retry_count = 0;

        loop {
            match self
                .context
                .client
                .get_live_room_stream_url(self.context.room_id, self.context.quality.to_quality())
                .await
            {
                Ok(stream_info) => return Ok(stream_info),
                Err(e) => {
                    retry_count += 1;
                    let delay = self.calculate_backoff_delay(retry_count);

                    eprintln!(
                        "获取直播流地址失败，正在重试 (第{retry_count}次，等待{delay:?}): {e}"
                    );

                    // 使用指数退避重试，无限重试
                    std::thread::sleep(delay);
                }
            }
        }
    }

    fn parse_stream_url(
        &self,
        stream_info: &LiveRoomStreamUrl,
    ) -> Result<(String, DownloaderType)> {
        let playurl_info = stream_info
            .playurl_info
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("未找到播放信息"))?;

        // 优先尝试http_hls协议
        if let Some(stream) = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == LiveProtocol::default())
        {
            return self.parse_hls_stream(stream);
        }

        // 如果没有http_hls，尝试http_stream协议
        if let Some(stream) = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == LiveProtocol::HttpStream)
        {
            return self.parse_http_stream(stream);
        }

        anyhow::bail!("未找到合适的直播流协议");
    }

    fn parse_http_stream(&self, stream: &PlayStream) -> Result<(String, DownloaderType)> {
        if stream.format.is_empty() {
            anyhow::bail!("未找到合适的直播流");
        }

        // 优先选择配置中的格式
        let format_stream = stream
            .format
            .iter()
            .find(|format| format.format_name == self.context.format)
            .or_else(|| stream.format.first())
            .ok_or_else(|| anyhow::anyhow!("未找到合适的视频格式"))?;

        if format_stream.codec.is_empty() {
            anyhow::bail!("未找到合适的视频编码");
        }

        // 优先按照设置选择编码格式
        let codec = format_stream
            .codec
            .iter()
            .find(|codec| codec.codec_name == self.context.codec)
            .unwrap_or_else(|| format_stream.codec.first().unwrap());

        // 随机选择URL
        let url_info = &codec.url_info[rand::rng().random_range(0..codec.url_info.len())];
        let url = format!("{}{}{}", url_info.host, codec.base_url, url_info.extra);

        let config = DownloadConfig {
            output_path: String::new(), // 将在start_download中设置
            overwrite: false,
            timeout: 30,
            retry_count: 3,
            codec: self.context.codec,
            format: self.context.format,
            quality: self.context.quality,
        };
        let http_downloader = HttpStreamDownloader::new(
            url.clone(),
            config,
            self.context.client.clone(),
            self.context.clone(),
        );

        Ok((url, DownloaderType::HttpStream(http_downloader)))
    }

    fn parse_hls_stream(&self, stream: &PlayStream) -> Result<(String, DownloaderType)> {
        if stream.format.is_empty() {
            anyhow::bail!("未找到合适的HLS流");
        }

        // 优先选择配置中的格式
        let format_stream = stream
            .format
            .iter()
            .find(|format| format.format_name == self.context.format)
            .or_else(|| stream.format.first())
            .ok_or_else(|| anyhow::anyhow!("未找到合适的视频格式"))?;

        if format_stream.codec.is_empty() {
            anyhow::bail!("未找到合适的视频编码");
        }

        // 优先按照设置选择编码格式
        let codec = format_stream
            .codec
            .iter()
            .find(|codec| codec.codec_name == self.context.codec)
            .unwrap_or_else(|| format_stream.codec.first().unwrap());

        // 随机选择URL
        let url_info = &codec.url_info[rand::rng().random_range(0..codec.url_info.len())];
        let url = format!("{}{}{}", url_info.host, codec.base_url, url_info.extra);

        // 创建HttpHlsDownloader
        let config = DownloadConfig {
            output_path: String::new(), // 将在start_download中设置
            overwrite: false,
            timeout: 30,
            retry_count: 3,
            codec: self.context.codec,
            format: self.context.format,
            quality: self.context.quality,
        };
        let hls_downloader = HttpHlsDownloader::new(url.clone(), config, self.context.clone());

        Ok((url, DownloaderType::HttpHls(hls_downloader)))
    }

    fn generate_filename(
        &self,
        room_info: &LiveRoomInfoData,
        user_info: &LiveUserInfo,
    ) -> Result<String> {
        let template = leon::Template::parse(DEFAULT_RECORD_NAME)
            .unwrap_or_else(|_| leon::Template::parse("{up_name}_{datetime}").unwrap());

        let live_time = NaiveDateTime::parse_from_str(&room_info.live_time, "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default();
        let live_time = live_time.and_local_timezone(Shanghai).unwrap();

        let values = DownloaderFilenameTemplate {
            up_name: user_info.uname.clone(),
            room_id: room_info.room_id,
            datetime: live_time.format("%Y-%m-%d %H点%M分").to_string(),
            room_title: room_info.title.clone(),
            room_description: room_info.description.clone(),
            room_area_name: room_info.area_name.clone(),
            date: live_time.format("%Y-%m-%d").to_string(),
        };

        let filename = template.render(&values).unwrap_or_default();
        Ok(filename)
    }

    fn resolve_file_path(&self, base_path: &str, filename: &str, ext: &str) -> Result<String> {
        const MAX_PARTS: u32 = 50; // 最大分片数量限制

        let initial_file_path = format!("{base_path}/{filename}.{ext}");
        let file_stem = std::path::Path::new(filename)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let folder_path = format!("{base_path}/{file_stem}");

        // 检查是否已经存在分P文件夹
        let folder_exists = std::path::Path::new(&folder_path).exists();
        let initial_file_exists = std::path::Path::new(&initial_file_path).exists();

        // 如果文件夹和原文件都不存在，返回原始路径
        if !folder_exists && !initial_file_exists {
            return Ok(initial_file_path);
        }

        // 如果存在分P文件夹或原文件存在，需要使用分P系统
        if folder_exists || initial_file_exists {
            // 创建文件夹（如果不存在）
            std::fs::create_dir_all(&folder_path).context("无法创建文件夹")?;

            // 扫描文件夹中现有的分P文件，找到所有现有的编号
            let mut existing_parts = Vec::new();

            if let Ok(folder) = std::fs::read_dir(&folder_path) {
                for entry in folder.flatten() {
                    let file_name_os = entry.file_name();
                    let file_name = file_name_os.to_string_lossy();

                    // 检查是否是我们的分P文件格式: {file_stem}_P{number}.{ext}
                    if let Some(name_without_ext) = file_name.strip_suffix(&format!(".{ext}")) {
                        if let Some(part_str) =
                            name_without_ext.strip_prefix(&format!("{file_stem}_P"))
                        {
                            // 尝试解析分P编号
                            if let Ok(part_num) = part_str.parse::<u32>() {
                                existing_parts.push(part_num);
                            }
                        }
                    }
                }
            }

            // 找到下一个可用的编号，但不超过最大限制
            let next_part_number = if existing_parts.is_empty() {
                1
            } else {
                existing_parts.sort();
                let max_existing = *existing_parts.iter().max().unwrap_or(&0);

                // 如果已达到最大分片数量，使用最后一个分片（P50）
                if max_existing >= MAX_PARTS {
                    MAX_PARTS
                } else {
                    max_existing + 1
                }
            };

            // 如果原文件存在且P1文件不存在，将原文件重命名为P1
            let first_part_name = format!("{file_stem}_P1.{ext}");
            let first_part_path = format!("{folder_path}/{first_part_name}");

            if initial_file_exists && !std::path::Path::new(&first_part_path).exists() {
                std::fs::rename(&initial_file_path, &first_part_path).context(format!(
                    "重命名原文件失败: {initial_file_path} -> {first_part_path}"
                ))?;
            }

            // 返回分P文件路径
            let new_file_name = format!("{file_stem}_P{next_part_number}.{ext}");
            let new_file_path = format!("{folder_path}/{new_file_name}");

            // 如果达到最大分片数量，记录日志提示
            if next_part_number == MAX_PARTS && existing_parts.contains(&MAX_PARTS) {
                eprintln!(
                    "⚠️  已达到最大分片数量({MAX_PARTS})，后续内容将附加到 P{MAX_PARTS} 文件中"
                );
            }

            Ok(new_file_path)
        } else {
            Ok(initial_file_path)
        }
    }

    pub async fn start_download(
        &mut self,
        cx: &mut AsyncApp,
        room_info: &LiveRoomInfoData,
        user_info: &LiveUserInfo,
        record_dir: &str,
    ) -> Result<()> {
        // 设置运行状态
        self.context.set_running(true);

        // 启动事件处理器
        self.context.start_event_processor(cx);

        // 获取流信息
        let stream_info = self.get_stream_info().await?;

        // 解析下载URL和选择下载器类型
        let (url, downloader_type) = self.parse_stream_url(&stream_info)?;

        // 生成文件名
        let filename = self.generate_filename(room_info, user_info)?;

        // 获取文件扩展名
        let ext = self.context.format.ext();

        // 处理文件路径冲突
        let file_path = self.resolve_file_path(record_dir, &filename, ext)?;

        // 根据下载器类型创建具体的下载器
        let mut final_downloader = match downloader_type {
            DownloaderType::HttpStream(_) => {
                let config = DownloadConfig {
                    output_path: file_path.clone(),
                    overwrite: false,
                    timeout: 30,
                    retry_count: 3,
                    codec: self.context.codec,
                    format: self.context.format,
                    quality: self.context.quality,
                };
                let downloader = HttpStreamDownloader::new(
                    url,
                    config,
                    self.context.client.clone(),
                    self.context.clone(),
                );

                DownloaderType::HttpStream(downloader)
            }
            DownloaderType::HttpHls(_) => {
                let config = DownloadConfig {
                    output_path: file_path.clone(),
                    overwrite: false,
                    timeout: 30,
                    retry_count: 3,
                    codec: self.context.codec,
                    format: self.context.format,
                    quality: self.context.quality,
                };
                let downloader = HttpHlsDownloader::new(url, config, self.context.clone());

                DownloaderType::HttpHls(downloader)
            }
        };

        match &mut final_downloader {
            DownloaderType::HttpStream(downloader) => match downloader.start(cx) {
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            },
            DownloaderType::HttpHls(downloader) => match downloader.start(cx) {
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            },
        }

        self.downloader = Some(final_downloader);

        Ok(())
    }

    /// 检查是否为网络相关错误
    fn is_network_error(error: &anyhow::Error) -> bool {
        let error_str = error.to_string().to_lowercase();

        // 检查常见的网络错误关键词
        error_str.contains("network")
            || error_str.contains("connection")
            || error_str.contains("timeout")
            || error_str.contains("dns")
            || error_str.contains("socket")
            || error_str.contains("unreachable")
            || error_str.contains("reset")
            || error_str.contains("refused")
            || error_str.contains("无法连接")
            || error_str.contains("连接被重置")
            || error_str.contains("连接超时")
            || error_str.contains("网络")
            || error_str.contains("请求失败")
            || error_str.contains("无法读取响应体")
    }

    /// 带重连的下载方法
    pub async fn start_download_with_retry(
        &mut self,
        cx: &mut AsyncApp,
        room_info: &LiveRoomInfoData,
        user_info: &LiveUserInfo,
        record_dir: &str,
    ) -> Result<()> {
        let mut retry_count = 0;

        loop {
            match self
                .start_download(cx, room_info, user_info, record_dir)
                .await
            {
                Ok(_) => {
                    // 下载成功启动，重置重连计数
                    self.context.update_stats(|stats| {
                        stats.reconnect_count = 0;
                    });

                    // 更新UI状态为录制中
                    self.update_card_status(cx, RoomCardStatus::Recording(0.0));

                    // 下载成功启动，现在监控下载状态
                    if self.is_auto_reconnect {
                        // 启动状态监控，处理自动重连和状态管理
                        self.monitor_download_status(cx, room_info, user_info, record_dir)
                            .await?;
                    }
                    return Ok(());
                }
                Err(e) if Self::is_network_error(&e) => {
                    retry_count += 1;
                    self.context.update_stats(|stats| {
                        stats.reconnect_count = retry_count;
                    });

                    let delay = self.calculate_backoff_delay(retry_count);

                    eprintln!("网络异常，正在尝试重连 (第{retry_count}次，等待{delay:?}): {e}");

                    // 更新UI状态显示重连信息
                    self.update_card_status(
                        cx,
                        RoomCardStatus::Error(format!(
                            "网络中断，第{}次重连 ({}秒后)",
                            retry_count,
                            delay.as_secs()
                        )),
                    );

                    // 发送重连事件
                    self.context.push_event(DownloadEvent::Reconnecting {
                        attempt: retry_count,
                        delay_secs: delay.as_secs(),
                    });

                    // 等待一段时间后重试 - 使用异步定时器
                    cx.background_executor().timer(delay).await;
                    continue;
                }
                Err(e) => {
                    // 非网络错误，直接返回
                    eprintln!("非网络错误，停止重连: {e}");

                    // 更新UI状态显示错误
                    self.update_card_status(cx, RoomCardStatus::Error(format!("录制失败: {e}")));

                    // 发送错误事件
                    self.context.push_event(DownloadEvent::Error {
                        error: DownloaderError::InvalidRecordingConfig {
                            field: "stream_url".to_string(),
                            value: "unavailable".to_string(),
                            reason: format!("非网络错误: {e}"),
                        },
                    });

                    return Err(e);
                }
            }
        }
    }

    pub fn stop(&mut self) {
        // 设置停止状态
        self.context.set_running(false);

        // 停止下载器
        if let Some(ref mut downloader) = self.downloader {
            match downloader {
                DownloaderType::HttpStream(downloader) => {
                    let _ = downloader.stop();
                }
                DownloaderType::HttpHls(downloader) => {
                    let _ = downloader.stop();
                }
            }
        }
    }

    /// 监控下载状态，根据事件处理重连或停止
    pub async fn monitor_download_status(
        &mut self,
        cx: &mut AsyncApp,
        room_info: &LiveRoomInfoData,
        user_info: &LiveUserInfo,
        record_dir: &str,
    ) -> Result<()> {
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 5;

        while self.context.is_running() {
            // 检查下载器状态
            if let Some(ref downloader) = self.downloader {
                let status = match downloader {
                    DownloaderType::HttpStream(downloader) => downloader.status(),
                    DownloaderType::HttpHls(downloader) => downloader.status(),
                };

                match status {
                    DownloadStatus::Error(error) => {
                        consecutive_errors += 1;

                        // 判断是否为网络错误
                        let is_network_error =
                            Self::is_network_error(&anyhow::anyhow!("{}", error));

                        if is_network_error
                            && consecutive_errors <= MAX_CONSECUTIVE_ERRORS
                            && self.is_auto_reconnect
                        {
                            // 发送错误事件（可恢复）
                            self.context.push_event(DownloadEvent::Error {
                                error: DownloaderError::NetworkError(error.clone()),
                            });

                            // 停止当前下载器
                            self.stop();

                            // 计算退避延迟
                            let delay = self.calculate_backoff_delay(consecutive_errors);

                            // 发送重连事件
                            self.context.push_event(DownloadEvent::Reconnecting {
                                attempt: consecutive_errors,
                                delay_secs: delay.as_secs(),
                            });

                            // 等待后重新启动下载
                            cx.background_executor().timer(delay).await;

                            match self
                                .start_download(cx, room_info, user_info, record_dir)
                                .await
                            {
                                Ok(_) => {
                                    consecutive_errors = 0; // 重置错误计数
                                    eprintln!("✅ 重连成功");
                                }
                                Err(e) => {
                                    eprintln!("❌ 重连失败: {e}");
                                }
                            }
                        } else {
                            // 不可恢复错误或超过最大重试次数
                            self.context.push_event(DownloadEvent::Error {
                                error: DownloaderError::NetworkError(format!(
                                    "连续错误超过{MAX_CONSECUTIVE_ERRORS}次，停止重连: {error}"
                                )),
                            });

                            self.stop();
                            break;
                        }
                    }
                    DownloadStatus::Completed => {
                        // 下载完成
                        if let Some(stats) = self.get_download_stats() {
                            self.context.push_event(DownloadEvent::Completed {
                                file_path: "".to_string(), // 具体路径由下载器提供
                                file_size: stats.bytes_downloaded,
                            });
                        }
                        break;
                    }
                    DownloadStatus::Downloading => {
                        consecutive_errors = 0; // 重置错误计数

                        // 更新进度
                        if let Some(stats) = self.get_download_stats() {
                            self.context.push_event(DownloadEvent::Progress {
                                bytes_downloaded: stats.bytes_downloaded,
                                download_speed_kbps: stats.download_speed_kbps,
                                duration_ms: stats.duration_ms,
                            });
                        }
                    }
                    DownloadStatus::Reconnecting => {
                        // 下载器内部正在重连，保持等待
                    }
                    DownloadStatus::NotStarted => {
                        // 下载器未启动，可能需要重新启动
                        eprintln!("⚠️  下载器未启动，尝试重新启动");
                        match self
                            .start_download(cx, room_info, user_info, record_dir)
                            .await
                        {
                            Ok(_) => {
                                eprintln!("✅ 重新启动成功");
                            }
                            Err(e) => {
                                eprintln!("❌ 重新启动失败: {e}");
                                consecutive_errors += 1;
                            }
                        }
                    }
                }
            }

            // 等待一段时间后再次检查
            cx.background_executor().timer(Duration::from_secs(2)).await;
        }

        Ok(())
    }

    /// 获取下载统计信息
    fn get_download_stats(&self) -> Option<DownloadStats> {
        self.downloader.as_ref().map(|downloader| match downloader {
            DownloaderType::HttpStream(downloader) => downloader.stats(),
            DownloaderType::HttpHls(downloader) => downloader.stats(),
        })
    }
}
