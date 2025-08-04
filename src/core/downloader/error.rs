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

    #[error("FFmpeg进程启动失败: {command} - {stderr}")]
    StartupFailed { command: String, stderr: String },

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
            DownloaderError::StartupFailed { .. } => true,
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
            DownloaderError::StartupFailed { .. }
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

            DownloaderError::StartupFailed { .. }
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
        Self::StartupFailed {
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
