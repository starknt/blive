pub mod http_hls;
pub mod http_stream;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 下载器特征
pub trait Downloader {
    /// 开始下载
    fn start(&mut self) -> Result<()>;

    /// 停止下载
    fn stop(&mut self) -> Result<()>;

    /// 获取下载状态
    fn status(&self) -> DownloadStatus;
}

/// 下载状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DownloadStatus {
    /// 未开始
    NotStarted,
    /// 下载中
    Downloading,
    /// 已暂停
    Paused,
    /// 已完成
    Completed,
    /// 出错
    Error(String),
}

/// 下载配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    /// 输出文件路径
    pub output_path: String,
    /// 是否覆盖已存在的文件
    pub overwrite: bool,
    /// 下载超时时间（秒）
    pub timeout: u64,
    /// 重试次数
    pub retry_count: u32,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            output_path: String::new(),
            overwrite: false,
            timeout: 30,
            retry_count: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use reqwest_client::ReqwestClient;

    use crate::api::HttpClient;

    use super::*;

    // 测试 hls
    // #[tokio::test]
    // async fn test_hls() {
    //     let downloader = http_hls::HttpHlsDownloader::new(
    //         "https://www.bilibili.com/video/BV1Qy4y1o71y".to_string(),
    //         DownloadConfig::default(),
    //     );
    // }

    // 测试 stream
    #[tokio::test]
    async fn test_stream() {
        let http_client =
            HttpClient::new(Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap()));

        let res = http_client
            .get_live_room_stream_url(1804892069, 10000)
            .await;

        if let Ok(_stream_url) = res {
            let mut downloader = http_stream::HttpStreamDownloader::new(
                "https://www.bilibili.com/video/BV1Qy4y1o71y".to_string(),
                DownloadConfig {
                    output_path: "test.flv".to_string(),
                    overwrite: true,
                    timeout: 30,
                    retry_count: 3,
                },
                http_client.clone(),
            );

            downloader.start().unwrap();
        }
    }
}
