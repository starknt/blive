use crate::components::{RoomCard, RoomCardStatus};
use crate::core::downloader::{DownloadConfig, DownloadStatus, Downloader};
use crate::core::http_client::HttpClient;
use anyhow::{Context, Result};
use futures_util::AsyncReadExt;
use gpui::http_client::{AsyncBody, Method, Request};
use gpui::{AsyncApp, WeakEntity};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub struct HttpStreamDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    client: HttpClient,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    entity: WeakEntity<RoomCard>,
}

impl HttpStreamDownloader {
    pub fn new(
        url: String,
        config: DownloadConfig,
        client: HttpClient,
        entity: WeakEntity<RoomCard>,
    ) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
            client,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            entity,
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

impl Downloader for HttpStreamDownloader {
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let url = self.url.clone();
        let config = self.config.clone();
        let client = self.client.clone();
        let is_running = self.is_running.clone();
        let entity = self.entity.clone();
        // 检查输出路径
        self.check_output_path()?;

        // 更新状态
        self.status = DownloadStatus::Downloading;
        is_running.store(true, std::sync::atomic::Ordering::Relaxed);

        cx.spawn(async move |cx| {
            #[cfg(debug_assertions)]
            eprintln!("开始下载: {url} 到 {}", config.output_path);
            if let Err(e) = Self::download_stream(&url, &config, &client, &is_running).await {
                #[cfg(debug_assertions)]
                eprintln!("下载失败: {e}");

                let _ = entity.update(cx, |card, cx| {
                    card.status = RoomCardStatus::Error;
                    card.error_message = Some(format!("下载失败: {e:?}"));
                    cx.notify();
                });

                return Err(e);
            }

            Ok(())
        })
        .detach();

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
    async fn download_stream(
        url: &str,
        config: &DownloadConfig,
        client: &HttpClient,
        is_running: &Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<()> {
        let mut retry_count = 0;
        let initial_delay = std::time::Duration::from_secs(1);

        loop {
            match Self::try_download_stream(url, config, client, is_running).await {
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
                            if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
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
                            if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
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
        is_running: &Arc<std::sync::atomic::AtomicBool>,
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
        const MAX_CONSECUTIVE_ERRORS: u32 = 3;

        loop {
            if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
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
