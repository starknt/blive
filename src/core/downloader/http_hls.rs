use crate::api::HttpClient;
use crate::core::downloader::{DownloadConfig, DownloadStatus, Downloader};
use anyhow::{Context, Result};
use futures_util::AsyncReadExt;
use gpui::http_client::{AsyncBody, Method, Request};
use gpui::{AsyncApp, Task};
use std::path::Path;
use std::sync::Arc;

/// HLS下载器
#[derive(Debug)]
pub struct HttpHlsDownloader {
    playlist_url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    client: HttpClient,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    task: Option<Task<()>>,
}

impl HttpHlsDownloader {
    /// 创建新的HLS下载器
    pub fn new(playlist_url: String, config: DownloadConfig, client: HttpClient) -> Self {
        Self {
            playlist_url,
            config,
            status: DownloadStatus::NotStarted,
            client,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            task: None,
        }
    }

    /// 解析m3u8播放列表
    #[allow(dead_code)]
    async fn parse_playlist(&self, playlist_content: &str) -> Result<Vec<String>> {
        let mut segments = Vec::new();
        let base_url = self.get_base_url(&self.playlist_url);

        for line in playlist_content.lines() {
            let line = line.trim();
            if !line.starts_with('#') && !line.is_empty() {
                let segment_url = if line.starts_with("http") {
                    line.to_string()
                } else {
                    format!("{base_url}{line}")
                };
                segments.push(segment_url);
            }
        }

        Ok(segments)
    }

    /// 获取基础URL
    #[allow(dead_code)]
    fn get_base_url(&self, url: &str) -> String {
        if let Some(last_slash) = url.rfind('/') {
            url[..=last_slash].to_string()
        } else {
            url.to_string()
        }
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

impl Downloader for HttpHlsDownloader {
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let playlist_url = self.playlist_url.clone();
        let config = self.config.clone();
        let client = self.client.clone();
        let is_running = self.is_running.clone();

        // 检查输出路径
        self.check_output_path()?;

        // 更新状态
        self.status = DownloadStatus::Downloading;
        is_running.store(true, std::sync::atomic::Ordering::Relaxed);

        let task = cx.background_executor().spawn(async move {
            if let Err(e) = Self::download_hls(&playlist_url, &config, &client).await {
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

impl HttpHlsDownloader {
    /// 执行HLS下载任务
    #[allow(dead_code)]
    async fn download_hls(
        playlist_url: &str,
        config: &DownloadConfig,
        client: &HttpClient,
    ) -> Result<()> {
        // 获取播放列表内容
        let request = Request::builder()
            .uri(playlist_url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut playlist_response = client.send(request).await.context("无法获取播放列表")?;

        if !playlist_response.status().is_success() {
            anyhow::bail!("获取播放列表失败: {}", playlist_response.status());
        }

        let mut playlist_content = String::new();
        playlist_response
            .body_mut()
            .read_to_string(&mut playlist_content)
            .await
            .context("无法读取播放列表内容")?;

        // 解析播放列表
        let segments = Self::parse_playlist_static(&playlist_content).await?;

        if segments.is_empty() {
            anyhow::bail!("播放列表为空");
        }

        // 创建输出文件
        let mut output_file =
            std::fs::File::create(&config.output_path).context("无法创建输出文件")?;

        // 下载所有片段
        for (index, segment_url) in segments.iter().enumerate() {
            let segment_request = Request::builder()
                .uri(segment_url)
                .method(Method::GET)
                .body(AsyncBody::empty())
                .context("Failed to build request")?;

            let mut segment_response = client
                .send(segment_request)
                .await
                .context(format!("无法下载片段 {index}"))?;

            if !segment_response.status().is_success() {
                eprintln!("下载片段 {} 失败: {}", index, segment_response.status());
                continue;
            }

            let mut segment_data = Vec::new();
            segment_response
                .body_mut()
                .read_to_end(&mut segment_data)
                .await
                .context(format!("无法读取片段 {index} 数据"))?;

            // 写入片段数据
            std::io::copy(&mut segment_data.as_slice(), &mut output_file)
                .context(format!("写入片段 {index} 失败"))?;
        }

        Ok(())
    }

    /// 解析播放列表（静态方法）
    #[allow(dead_code)]
    async fn parse_playlist_static(playlist_content: &str) -> Result<Vec<String>> {
        let mut segments = Vec::new();
        let lines: Vec<&str> = playlist_content.lines().collect();

        for line in lines {
            let line = line.trim();
            if !line.starts_with('#') && !line.is_empty() {
                segments.push(line.to_string());
            }
        }

        Ok(segments)
    }
}
