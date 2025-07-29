use crate::api::HttpClient;
use crate::core::downloader::{DownloadConfig, DownloadStatus, Downloader};
use anyhow::{Context, Result};
use futures_util::AsyncReadExt;
use gpui::http_client::{AsyncBody, Method, Request};
use std::path::Path;
use std::sync::Arc;

/// HTTP流下载器
pub struct HttpStreamDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    client: HttpClient,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl HttpStreamDownloader {
    /// 创建新的HTTP流下载器
    pub fn new(url: String, config: DownloadConfig, client: HttpClient) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
            client,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 验证URL是否有效
    pub async fn validate_url(&self) -> Result<()> {
        let request = Request::builder()
            .uri(&self.url)
            .method(Method::HEAD)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let response = self.client.send(request).await.context("无法连接到URL")?;

        if !response.status().is_success() {
            anyhow::bail!("URL返回错误状态: {}", response.status());
        }

        Ok(())
    }

    /// 检查输出路径
    fn check_output_path(&self) -> Result<()> {
        let path = Path::new(&self.config.output_path);

        if path.exists() && !self.config.overwrite {
            anyhow::bail!("输出文件已存在且不允许覆盖");
        }

        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).context("无法创建输出目录")?;
        }

        Ok(())
    }
}

impl Downloader for HttpStreamDownloader {
    fn start(&mut self) -> Result<()> {
        let url = self.url.clone();
        let _config = self.config.clone();
        let _client = self.client.clone();
        let is_running = self.is_running.clone();

        // 检查输出路径
        self.check_output_path()?;

        // 更新状态
        self.status = DownloadStatus::Downloading;
        is_running.store(true, std::sync::atomic::Ordering::Relaxed);

        // 启动异步下载任务
        std::thread::spawn(move || {
            // 这里我们使用同步的方式，因为GPUI的异步运行时可能不支持spawn
            // 在实际应用中，应该通过GPUI的异步API来处理
            eprintln!("开始下载: {url}");
        });

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.status = DownloadStatus::Paused;
        Ok(())
    }

    fn status(&self) -> DownloadStatus {
        self.status.clone()
    }
}

impl HttpStreamDownloader {
    /// 执行实际的下载任务
    #[allow(dead_code)]
    async fn download_stream(
        url: &str,
        config: &DownloadConfig,
        client: &HttpClient,
    ) -> Result<()> {
        let request = Request::builder()
            .uri(url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = client.send(request).await.context("请求失败")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP请求失败: {}", response.status());
        }

        let mut file = std::fs::File::create(&config.output_path).context("无法创建输出文件")?;

        // 获取响应体
        let body = response.body_mut();
        let mut buffer = Vec::new();
        body.read_to_end(&mut buffer)
            .await
            .context("无法读取响应体")?;

        // 写入文件
        std::io::copy(&mut buffer.as_slice(), &mut file).context("写入文件失败")?;

        Ok(())
    }
}
