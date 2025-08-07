#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DownloaderError {
    // 网络相关错误
    #[error("网络连接失败: {message}")]
    NetworkConnectionFailed { message: String },

    #[error("网络错误: {0}")]
    NetworkError(String),

    // 流媒体相关错误
    #[error("流编码错误: {codec} - {details}")]
    StreamEncodingError { codec: String, details: String },

    #[error("流错误: {0}")]
    StreamError(String),

    // FFmpeg相关错误
    #[error("FFmpeg进程启动失败: {command} - {stderr}")]
    StartupFailed { command: String, stderr: String },

    #[error("FFmpeg运行时错误: {error_type} - {message}")]
    FfmpegRuntimeError { error_type: String, message: String },

    #[error("FFmpeg错误: {0}")]
    FfmpegError(String),

    // 文件系统相关错误
    #[error("文件创建失败: {path} - {reason}")]
    FileCreationFailed { path: String, reason: String },

    #[error("文件系统错误: {0}")]
    FileSystemError(String),

    // 配置相关错误
    #[error("无效的录制配置: {field} = {value} ({reason})")]
    InvalidRecordingConfig {
        field: String,
        value: String,
        reason: String,
    },

    #[error("配置错误: {0}")]
    ConfigError(String),
}

impl DownloaderError {
    /// 判断错误是否可恢复
    pub fn is_recoverable(&self) -> bool {
        match self {
            DownloaderError::NetworkConnectionFailed { .. } => true,
            DownloaderError::NetworkError(_) => true,
            DownloaderError::StreamEncodingError { .. } => true,
            DownloaderError::StreamError(_) => true,
            DownloaderError::StartupFailed { .. } => true,
            DownloaderError::FfmpegRuntimeError { .. } => true,
            DownloaderError::FfmpegError(_) => false,
            DownloaderError::FileCreationFailed { .. } => false,
            DownloaderError::FileSystemError(_) => false,
            DownloaderError::InvalidRecordingConfig { .. } => false,
            DownloaderError::ConfigError(_) => false,
        }
    }

    /// 获取错误的严重程度
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // 网络错误通常是临时的
            DownloaderError::NetworkConnectionFailed { .. } | DownloaderError::NetworkError(_) => {
                ErrorSeverity::Warning
            }

            // 流相关错误通常是警告
            DownloaderError::StreamEncodingError { .. } | DownloaderError::StreamError(_) => {
                ErrorSeverity::Warning
            }

            // FFmpeg错误根据类型判断
            DownloaderError::StartupFailed { .. } => ErrorSeverity::Critical,
            DownloaderError::FfmpegRuntimeError { .. } => ErrorSeverity::Error,
            DownloaderError::FfmpegError(_) => ErrorSeverity::Error,

            // 文件系统错误通常是严重的
            DownloaderError::FileCreationFailed { .. } | DownloaderError::FileSystemError(_) => {
                ErrorSeverity::Error
            }

            // 配置错误
            DownloaderError::InvalidRecordingConfig { .. } | DownloaderError::ConfigError(_) => {
                ErrorSeverity::Error
            }
        }
    }

    /// 获取错误分类
    pub fn category(&self) -> ErrorCategory {
        match self {
            DownloaderError::NetworkConnectionFailed { .. } | DownloaderError::NetworkError(_) => {
                ErrorCategory::Network
            }

            DownloaderError::StreamEncodingError { .. } | DownloaderError::StreamError(_) => {
                ErrorCategory::Stream
            }

            DownloaderError::StartupFailed { .. }
            | DownloaderError::FfmpegRuntimeError { .. }
            | DownloaderError::FfmpegError(_) => ErrorCategory::Ffmpeg,

            DownloaderError::FileCreationFailed { .. } | DownloaderError::FileSystemError(_) => {
                ErrorCategory::FileSystem
            }

            DownloaderError::InvalidRecordingConfig { .. } | DownloaderError::ConfigError(_) => {
                ErrorCategory::Configuration
            }
        }
    }

    /// 便捷方法：创建网络连接失败错误
    pub fn network_connection_failed(message: impl Into<String>) -> Self {
        Self::NetworkConnectionFailed {
            message: message.into(),
        }
    }

    /// 便捷方法：创建网络错误
    pub fn network_error(message: impl Into<String>) -> Self {
        Self::NetworkError(message.into())
    }

    /// 便捷方法：创建流编码错误
    pub fn stream_encoding_error(codec: impl Into<String>, details: impl Into<String>) -> Self {
        Self::StreamEncodingError {
            codec: codec.into(),
            details: details.into(),
        }
    }

    /// 便捷方法：创建流错误
    pub fn stream_error(message: impl Into<String>) -> Self {
        Self::StreamError(message.into())
    }

    /// 便捷方法：创建FFmpeg启动失败错误
    pub fn ffmpeg_startup_failed(command: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self::StartupFailed {
            command: command.into(),
            stderr: stderr.into(),
        }
    }

    /// 便捷方法：创建FFmpeg运行时错误
    pub fn ffmpeg_runtime_error(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self::FfmpegRuntimeError {
            error_type: error_type.into(),
            message: message.into(),
        }
    }

    /// 便捷方法：创建FFmpeg错误
    pub fn ffmpeg_error(message: impl Into<String>) -> Self {
        Self::FfmpegError(message.into())
    }

    /// 便捷方法：创建文件创建失败错误
    pub fn file_creation_failed(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::FileCreationFailed {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// 便捷方法：创建文件系统错误
    pub fn file_system_error(message: impl Into<String>) -> Self {
        Self::FileSystemError(message.into())
    }

    /// 便捷方法：创建配置错误
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError(message.into())
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
}
