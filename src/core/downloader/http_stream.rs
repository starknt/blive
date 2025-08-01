use crate::core::downloader::{
    DownloadConfig, DownloadEvent, DownloadStats, DownloadStatus, Downloader, DownloaderContext,
};
use crate::core::http_client::HttpClient;
use anyhow::{Context, Result};
use futures::AsyncReadExt;
use gpui::AsyncApp;
use gpui::http_client::{AsyncBody, Method, Request};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct HttpStreamDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    client: HttpClient,
    stats: DownloadStats,
    start_time: Option<Instant>,
    context: DownloaderContext,
}

impl HttpStreamDownloader {
    pub fn new(
        url: String,
        config: DownloadConfig,
        client: HttpClient,
        context: DownloaderContext,
    ) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
            client,
            stats: DownloadStats::default(),
            start_time: None,
            context,
        }
    }

    /// 发送事件到队列
    fn emit_event(&self, event: DownloadEvent) {
        self.context.push_event(event);
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
        let context = self.context.clone();

        // 检查输出路径
        self.check_output_path()?;

        // 更新状态
        self.status = DownloadStatus::Downloading;
        self.start_time = Some(Instant::now());
        self.context.set_running(true);

        // 发送开始事件
        self.emit_event(DownloadEvent::Started {
            file_path: config.output_path.clone(),
        });

        cx.spawn(async move |_cx| {
            #[cfg(debug_assertions)]
            eprintln!("开始下载: {url} 到 {}", config.output_path);
            if let Err(e) = Self::download_stream(&url, &config, &client, &context).await {
                #[cfg(debug_assertions)]
                eprintln!("下载失败: {e}");

                return Err(e);
            }

            // 下载完成
            #[cfg(debug_assertions)]
            eprintln!("下载完成: {}", config.output_path);

            Ok(())
        })
        .detach();

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.context.set_running(false);
        self.status = DownloadStatus::Paused;

        self.emit_event(DownloadEvent::Paused);
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.context.set_running(false);
        self.status = DownloadStatus::Paused;

        self.emit_event(DownloadEvent::Paused);
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        self.context.set_running(true);
        self.status = DownloadStatus::Downloading;

        self.emit_event(DownloadEvent::Resumed);
        Ok(())
    }

    fn status(&self) -> DownloadStatus {
        self.status.clone()
    }

    fn stats(&self) -> DownloadStats {
        self.stats.clone()
    }
}

impl HttpStreamDownloader {
    async fn download_stream(
        url: &str,
        config: &DownloadConfig,
        client: &HttpClient,
        context: &DownloaderContext,
    ) -> Result<()> {
        let mut retry_count = 0;
        let initial_delay = std::time::Duration::from_secs(1);

        loop {
            match Self::try_download_stream(url, config, client, context).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    retry_count += 1;

                    // 检查是否是网络错误
                    let error_str = e.to_string().to_lowercase();
                    let is_network_error = error_str.contains("connection")
                        || error_str.contains("timeout")
                        || error_str.contains("reset")
                        || error_str.contains("unreachable")
                        || error_str.contains("请求失败")
                        || error_str.contains("无法读取响应体");

                    if is_network_error {
                        // 计算指数退避延迟，最大30分钟
                        const MAX_DELAY: std::time::Duration =
                            std::time::Duration::from_secs(30 * 60);
                        let exponential_delay = initial_delay * (2_u32.pow(retry_count.min(10)));
                        let delay = if exponential_delay > MAX_DELAY {
                            MAX_DELAY
                        } else {
                            exponential_delay
                        };

                        eprintln!("网络错误，正在重试 (第{retry_count}次，等待{delay:?}): {e}");

                        // 使用指数退避延迟
                        for _ in 0..(delay.as_millis() / 10) {
                            if !context.is_running() {
                                return Ok(());
                            }
                            std::thread::yield_now();
                        }
                        continue;
                    } else {
                        // 非网络错误，检查是否达到最大重试次数
                        if retry_count >= config.retry_count {
                            return Err(anyhow::anyhow!("下载失败，已达最大重试次数: {}", e));
                        }

                        // 对于非网络错误，使用较短的延迟
                        let delay = std::time::Duration::from_secs(2_u64.pow(retry_count.min(5)));
                        for _ in 0..(delay.as_millis() / 10) {
                            if !context.is_running() {
                                return Ok(());
                            }
                            std::thread::yield_now();
                        }
                    }
                }
            }
        }
    }

    async fn try_download_stream(
        url: &str,
        config: &DownloadConfig,
        client: &HttpClient,
        context: &DownloaderContext,
    ) -> Result<()> {
        let request = Request::builder()
            .uri(url)
            .header("User-Agent","Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Referer", "https://live.bilibili.com/")
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = client
            .send(request)
            .await
            .context("网络请求失败，可能是网络连接问题")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "HTTP请求失败: {}，可能是服务器错误或直播流已失效",
                response.status()
            );
        }

        let mut file = std::fs::File::create(&config.output_path).context("无法创建输出文件")?;

        // 获取响应体
        let body = response.body_mut();
        let mut buffer = [0; 8192];
        let mut consecutive_errors = 0;
        let mut bytes_downloaded = 0u64;
        let start_time = Instant::now();
        let mut last_progress_update = Instant::now();

        const MAX_CONSECUTIVE_ERRORS: u32 = 3;
        const PROGRESS_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

        loop {
            if !context.is_running() {
                return Ok(());
            }

            match body.read(&mut buffer).await {
                Ok(bytes_read) => {
                    if bytes_read == 0 {
                        // 流结束
                        return Ok(());
                    }

                    if let Err(e) = file.write_all(&buffer[..bytes_read]) {
                        return Err(anyhow::anyhow!("写入文件失败: {}", e));
                    }

                    bytes_downloaded += bytes_read as u64;

                    // 定期更新进度
                    let now = Instant::now();
                    if now.duration_since(last_progress_update) >= PROGRESS_UPDATE_INTERVAL {
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        let download_speed_kbps = if duration_ms > 0 {
                            (bytes_downloaded as f32 * 8.0) / (duration_ms as f32) // 转换为 kbps
                        } else {
                            0.0
                        };

                        // 发送进度事件到队列
                        context.push_event(DownloadEvent::Progress {
                            bytes_downloaded,
                            download_speed_kbps,
                            duration_ms,
                        });

                        last_progress_update = now;
                    }

                    // 重置连续错误计数
                    consecutive_errors = 0;
                }
                Err(e) => {
                    consecutive_errors += 1;
                    eprintln!(
                        "读取响应体时出现错误 ({consecutive_errors}/{MAX_CONSECUTIVE_ERRORS}): {e}"
                    );

                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        return Err(anyhow::anyhow!("无法读取响应体，连续错误次数过多: {}", e));
                    }

                    // 短暂等待后继续尝试 - 这里使用一个非常短的等待
                    // 在实际应用中，网络读取错误通常应该立即重试或者失败
                    // 这里的目的是避免busy loop
                    std::thread::yield_now();
                }
            }
        }
    }
}
