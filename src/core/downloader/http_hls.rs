#![allow(clippy::collapsible_if)]

use crate::components::RoomCard;
use crate::core::downloader::{DownloadConfig, DownloadStatus, Downloader};
use crate::settings::StreamCodec;
use anyhow::{Context, Result};
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use gpui::{AsyncApp, WeakEntity};
use std::sync::{Arc, Mutex};

pub struct HttpHlsDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    entity: WeakEntity<RoomCard>,
    inner: Arc<Mutex<Option<FfmpegChild>>>,
}

impl HttpHlsDownloader {
    pub fn new(url: String, config: DownloadConfig, entity: WeakEntity<RoomCard>) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            entity,
            inner: Arc::new(Mutex::new(None)),
        }
    }

    fn download_stream(url: &str, config: &DownloadConfig) -> Result<FfmpegChild> {
        let user_agent_header = format!(
            "User-Agent: {}",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        );
        let referer_header = format!("Referer: {}", "https://live.bilibili.com/");

        let mut cmd = FfmpegCommand::new();
        cmd.input(url);

        if config.overwrite {
            cmd.overwrite();
        } else {
            cmd.no_overwrite();
        }

        // GPU 加速
        if config.hwaccel {
            cmd.hwaccel("auto");
        }

        cmd.arg("-headers")
            .arg(user_agent_header)
            .arg("-headers")
            .arg(referer_header)
            .arg("-reconnect")
            .arg("1")
            .arg("-reconnect_on_network_error")
            .arg("1")
            .arg("-reconnect_on_http_error")
            .arg("5xx")
            .arg("-reconnect_at_eof")
            .arg("1")
            .arg("-reconnect_streamed")
            .arg("1")
            .arg("-reconnect_delay_max")
            .arg("2")
            .arg("-respect_retry_after")
            .arg("1")
            .arg("-xerror")
            .arg("-v")
            .arg("error")
            .args([
                "-c:v",
                match config.codec {
                    StreamCodec::AVC => "copy",
                    StreamCodec::HEVC => "hevc",
                },
            ])
            .args(["-c:a", "copy"])
            .args(["-bsf:a", "aac_adtstoasc"]);

        cmd.output(config.output_path.clone());

        let process = cmd.spawn()?;

        Ok(process)
    }
}

impl Downloader for HttpHlsDownloader {
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let url = self.url.clone();
        let config = self.config.clone();
        let entity = self.entity.clone();

        // 更新状态
        self.status = DownloadStatus::Downloading;
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let process = Self::download_stream(&url, &config).context("无法创建 FFmpeg 上下文")?;
        let mut lock = self.inner.lock().unwrap();
        *lock = Some(process);
        drop(lock);
        let process = self.inner.clone();
        let is_running = self.is_running.clone();
        cx.spawn(async move |cx| {
            if let Ok(mut guard) = process.lock() {
                if let Some(ref mut process) = *guard {
                    if let Ok(iter) = process.iter() {
                        for event in iter {
                            if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                                drop(guard);
                                break;
                            }

                            match event {
                                FfmpegEvent::Progress(progress) => {
                                    println!("progress: {progress:?}");
                                    let _ = entity.update(cx, |_, _| {
                                        // 进度
                                        todo!()
                                    });
                                }
                                FfmpegEvent::Done => {}
                                _ => {}
                            }
                        }
                    }
                }
            }
        })
        .detach();

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        // 设置停止标志
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.status = DownloadStatus::Paused;

        if let Ok(mut guard) = self.inner.lock() {
            if let Some(process) = guard.as_mut() {
                if let Err(e) = process.quit() {
                    eprintln!("FFmpeg 进程退出失败: {e}");
                }
            }
        }

        Ok(())
    }

    fn status(&self) -> DownloadStatus {
        self.status.clone()
    }
}
