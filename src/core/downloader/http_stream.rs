use crate::api::HttpClient;
use crate::core::downloader::{DownloadConfig, DownloadStatus, Downloader};
use anyhow::{Context, Result};
use futures_util::AsyncReadExt;
use gpui::http_client::{AsyncBody, Method, Request};
use gpui::{AsyncApp, Task};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

/// HTTP流下载器
#[derive(Debug)]
pub struct HttpStreamDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    client: HttpClient,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    task: Option<Task<()>>,
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
            task: None,
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
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let url = self.url.clone();
        let config = self.config.clone();
        let client = self.client.clone();
        let is_running = self.is_running.clone();

        // 检查输出路径
        self.check_output_path()?;

        // 更新状态
        self.status = DownloadStatus::Downloading;
        is_running.store(true, std::sync::atomic::Ordering::Relaxed);

        let task = cx.background_executor().spawn(async move {
            eprintln!("开始下载: {url}");
            if let Err(e) = Self::download_stream(&url, &config, &client, &is_running).await {
                eprintln!("下载失败: {e}");
            }
        });

        self.task = Some(task);

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.status = DownloadStatus::Paused;
        if let Some(task) = self.task.take() {
            task.detach();
        }
        Ok(())
    }

    fn status(&self) -> DownloadStatus {
        self.status.clone()
    }
}

impl HttpStreamDownloader {
    /// 执行实际的下载任务（静态方法）
    async fn download_stream(
        url: &str,
        config: &DownloadConfig,
        client: &HttpClient,
        is_running: &Arc<std::sync::atomic::AtomicBool>,
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
        let mut buffer = [0; 8192];

        loop {
            if let Ok(bytes_read) = body.read(&mut buffer).await {
                if bytes_read == 0 {
                    return Ok(());
                }

                let write_result = file.write_all(&buffer[..bytes_read]);

                if let Err(e) = write_result {
                    // 根据错误类型返回相应的 RecordError
                    return Err(e.into());
                }

                if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                    return Ok(());
                }
            } else {
                return Err(anyhow::anyhow!("无法读取响应体"));
            }
        }
    }
}
