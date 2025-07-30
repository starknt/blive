use crate::components::{RoomCard, RoomCardStatus};
use crate::core::downloader::{DownloadConfig, DownloadStatus, Downloader};
use crate::settings::StreamCodec;
use anyhow::{Context, Result};
use ez_ffmpeg::core::scheduler::ffmpeg_scheduler::{Initialization, Paused, Running};
use ez_ffmpeg::{FfmpegContext, FfmpegScheduler, Input, Output};
use gpui::{AsyncApp, WeakEntity};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct HttpHlsDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    entity: WeakEntity<RoomCard>,
    running_scheduler: Arc<Mutex<Option<FfmpegScheduler<Running>>>>,
    paused_scheduler: Arc<Mutex<Option<FfmpegScheduler<Paused>>>>,
}

impl HttpHlsDownloader {
    pub fn new(url: String, config: DownloadConfig, entity: WeakEntity<RoomCard>) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            entity,
            running_scheduler: Arc::new(Mutex::new(None)),
            paused_scheduler: Arc::new(Mutex::new(None)),
        }
    }

    fn download_stream(
        url: &str,
        config: &DownloadConfig,
    ) -> Result<FfmpegScheduler<Initialization>> {
        let mut input = Input::new(url);
        input = input.set_input_opts(vec![
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string()),
            ("Referer", "https://live.bilibili.com/".to_string()),
            ("reconnect", "1".to_string()),
            ("reconnect_at_eof", "1".to_string()),
            ("reconnect_streamed", "1".to_string()),
            ("reconnect_delay_max", "2".to_string()),
        ]);

        // 根据编码设置视频编码器
        match config.codec {
            StreamCodec::AVC => {
                input = input.set_video_codec("h264");
            }
            StreamCodec::HEVC => {
                input = input.set_video_codec("hevc");
            }
        }

        let output = Output::new(config.output_path.clone())
            .set_audio_codec("aac")
            .set_audio_channels(2)
            .set_video_codec(match config.codec {
                StreamCodec::AVC => "h264",
                StreamCodec::HEVC => "hevc",
            });

        let ctx = FfmpegContext::builder()
            .input(input)
            .output(output)
            .build()
            .context("无法创建 FFmpeg 上下文")?;

        Ok(FfmpegScheduler::new(ctx))
    }

    /// 确保输出目录存在
    fn ensure_output_directory(&self) -> Result<()> {
        let output_path = std::path::Path::new(&self.config.output_path);

        if let Some(parent) = output_path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).context("无法创建输出目录")?;
        }

        // 检查是否允许覆盖
        if output_path.exists() && !self.config.overwrite {
            anyhow::bail!("输出文件已存在且不允许覆盖: {}", self.config.output_path);
        }

        Ok(())
    }
}

impl Downloader for HttpHlsDownloader {
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let url = self.url.clone();
        let config = self.config.clone();
        let entity = self.entity.clone();
        let is_running = self.is_running.clone();
        let running_scheduler_ref = self.running_scheduler.clone();
        let paused_scheduler_ref = self.paused_scheduler.clone();

        // 确保输出目录存在
        self.ensure_output_directory()?;

        // 更新状态
        self.status = DownloadStatus::Downloading;
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        match Self::download_stream(&url, &config).context("无法创建 FFmpeg 上下文") {
            Ok(scheduler) => {
                match scheduler.start() {
                    Ok(scheduler) => {
                        {
                            let mut running_scheduler_guard = running_scheduler_ref.lock().unwrap();
                            *running_scheduler_guard = Some(scheduler);
                        }

                        cx.spawn(async move |cx| {
                            loop {
                                // 检查是否需要暂停
                                if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                                    if let Ok(mut running_scheduler_guard) =
                                        running_scheduler_ref.lock()
                                        && let Some(scheduler) = running_scheduler_guard.take()
                                    {
                                        if let Ok(mut paused_scheduler_guard) =
                                            paused_scheduler_ref.lock()
                                        {
                                            *paused_scheduler_guard = Some(scheduler.pause());
                                        }
                                        break;
                                    }
                                } else {
                                    // 检查是否完成
                                    if let Ok(running_scheduler_guard) =
                                        running_scheduler_ref.lock()
                                        && let Some(ref scheduler) = *running_scheduler_guard
                                        && scheduler.is_ended()
                                    {
                                        // 下载完成，更新状态
                                        is_running
                                            .store(false, std::sync::atomic::Ordering::Relaxed);
                                        break;
                                    }

                                    // 检查是否完成
                                    if let Ok(paused_scheduler_guard) = paused_scheduler_ref.lock()
                                        && let Some(ref scheduler) = *paused_scheduler_guard
                                        && scheduler.is_ended()
                                    {
                                        // 下载完成，更新状态
                                        is_running
                                            .store(false, std::sync::atomic::Ordering::Relaxed);
                                        break;
                                    }
                                }

                                cx.background_executor().timer(Duration::from_secs(3)).await;
                            }
                        })
                        .detach();
                    }
                    Err(e) => {
                        self.status = DownloadStatus::Error(e.to_string());
                        self.is_running
                            .store(false, std::sync::atomic::Ordering::Relaxed);
                        let _ = entity.update(cx, |card, _| {
                            card.status = RoomCardStatus::Error;
                            card.error_message = Some(e.to_string());
                        });
                        return Err(anyhow::anyhow!("无法启动 FFmpeg 上下文: {}", e));
                    }
                }
            }
            Err(e) => {
                self.status = DownloadStatus::Error(e.to_string());
                self.is_running
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                return Err(e);
            }
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        // 设置停止标志
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.status = DownloadStatus::Paused;

        if let Ok(mut running_scheduler_guard) = self.running_scheduler.lock()
            && let Some(scheduler) = running_scheduler_guard.take()
        {
            scheduler.abort();
        }

        if let Ok(mut paused_scheduler_guard) = self.paused_scheduler.lock()
            && let Some(scheduler) = paused_scheduler_guard.take()
        {
            scheduler.abort();
        }

        Ok(())
    }

    fn status(&self) -> DownloadStatus {
        self.status.clone()
    }
}
