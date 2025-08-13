#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DownloaderError {
    // 没有找到合适的直播流协议
    #[error("没有找到合适的直播流协议")]
    NoSuitableStreamProtocol,
    // 没有找到合适的视频格式
    #[error("没有找到合适的视频格式")]
    NoSuitableVideoFormat,
    // 没有找到合适的视频编码
    #[error("没有找到合适的视频编码")]
    NoSuitableVideoCodec,
    // 没有找到合适的音频编码
    #[error("没有找到合适的音频编码")]
    NoSuitableAudioCodec,

    #[error("进程启动失败: {command} - {stderr}")]
    StartupFailed { command: String, stderr: String },

    // 网络连接失败
    #[error("网络连接失败: {message}")]
    NetworkConnectionFailed { message: String },

    // ffmpeg 致命错误
    #[error("ffmpeg 致命错误: {message}")]
    FfmpegFatalError { message: String },

    // 文件系统相关错误
    #[error("文件创建失败: {path} - {reason}")]
    FileCreationFailed { path: String, reason: String },

    #[error("文件写入失败: {path} - {reason}")]
    FileWriteFailed { path: String, reason: String },

    // 配置相关错误
    #[error("无效的录制配置: {field} = {value} ({reason})")]
    InvalidRecordingConfig {
        field: String,
        value: String,
        reason: String,
    },
}

impl DownloaderError {
    /// 判断错误是否可恢复
    pub fn is_recoverable(&self) -> bool {
        match self {
            DownloaderError::NoSuitableStreamProtocol
            | DownloaderError::NoSuitableVideoFormat
            | DownloaderError::NoSuitableVideoCodec => true,
            DownloaderError::StartupFailed { .. } => true,
            _ => true,
        }
    }

    pub fn is_requires_restart(&self) -> bool {
        match self {
            DownloaderError::NetworkConnectionFailed { .. } => true,
            DownloaderError::FfmpegFatalError { .. } => true,
            _ => true,
        }
    }
}
