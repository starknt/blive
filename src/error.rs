use thiserror::Error;

/// 应用错误类型
#[derive(Error, Debug)]
pub enum AppError {
    /// API请求错误
    #[error("API请求失败: {0}")]
    ApiError(String),

    /// 网络错误
    #[error("网络错误: {0}")]
    NetworkError(String),

    /// 文件系统错误
    #[error("文件系统错误: {0}")]
    FileSystemError(String),

    /// 配置错误
    #[error("配置错误: {0}")]
    ConfigError(String),

    /// 下载错误
    #[error("下载错误: {0}")]
    DownloadError(String),

    /// 房间错误
    #[error("房间错误: {0}")]
    RoomError(String),

    /// 未知错误
    #[error("未知错误: {0}")]
    Unknown(String),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::FileSystemError(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::ConfigError(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Unknown(err.to_string())
    }
}

/// 结果类型别名
pub type AppResult<T> = Result<T, AppError>;
