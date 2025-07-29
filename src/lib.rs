pub mod api;
pub mod app; // 新增应用主逻辑模块
pub mod assets;
pub mod components;
pub mod config; // 新增配置管理模块
pub mod core;
pub mod error; // 新增错误处理模块
pub mod logger; // 新增日志管理模块
pub mod settings;
pub mod state;
pub mod themes;
pub mod title_bar;

// 重新导出主应用结构
pub use app::BLiveApp;

// 重新导出下载器
pub use core::downloader::{
    DownloadConfig, DownloadStatus, Downloader, http_hls::HttpHlsDownloader,
    http_stream::HttpStreamDownloader,
};
