#![allow(clippy::collapsible_if)]

use crate::core::downloader::{
    DownloadConfig, DownloadEvent, DownloadStats, DownloadStatus, Downloader, DownloaderContext,
};
use anyhow::{Context, Result};
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use futures::StreamExt;
use futures::channel::mpsc;
use gpui::AsyncApp;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct HttpHlsDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    inner: Arc<Mutex<Option<FfmpegChild>>>,
    stats: DownloadStats,
    start_time: Option<Instant>,
    context: DownloaderContext,
}

#[derive(Debug)]
enum HttpHlsDownloaderEvent {
    Progress(f32),
    Done,
    Error(String),
}

impl HttpHlsDownloader {
    pub fn new(url: String, config: DownloadConfig, context: DownloaderContext) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            inner: Arc::new(Mutex::new(None)),
            stats: DownloadStats::default(),
            start_time: None,
            context,
        }
    }

    /// 发送事件到队列
    fn emit_event(&self, event: DownloadEvent) {
        self.context.push_event(event);
    }

    fn download_stream(url: &str, config: &DownloadConfig) -> Result<FfmpegChild> {
        let user_agent_header = format!(
            "User-Agent: {}",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        );
        let referer_header = format!("Referer: {}", "https://live.bilibili.com/");

        let mut cmd = FfmpegCommand::new();

        cmd.arg("-headers")
            .arg(user_agent_header)
            .arg("-headers")
            .arg(referer_header)
            .arg("-i")
            .arg(url)
            // .arg("-xerror")
            // .arg("-v")
            // .arg("quiet")
            .arg("-c")
            .arg("copy")
            .arg("-bsf:a")
            .arg("aac_adtstoasc") // if using AAC in TS
            // .arg("-c:v")
            // .arg(match config.codec {
            //     StreamCodec::AVC => "copy",
            //     StreamCodec::HEVC => "hevc",
            // })
            // .arg("-c:a")
            // .arg("copy")
            .arg(config.output_path.clone());

        if config.overwrite {
            cmd.overwrite();
        } else {
            cmd.no_overwrite();
        }

        let process = cmd.spawn().unwrap();

        Ok(process)
    }
}

impl Downloader for HttpHlsDownloader {
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let url = self.url.clone();
        let config = self.config.clone();
        // 更新状态
        self.status = DownloadStatus::Downloading;
        self.start_time = Some(Instant::now());
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // 发送开始事件
        self.emit_event(DownloadEvent::Started {
            file_path: config.output_path.clone(),
        });

        let inner = self.inner.clone();
        let is_running = self.is_running.clone();
        let config_clone = config.clone();
        let (mut tx, mut rx) = mpsc::channel::<HttpHlsDownloaderEvent>(100);

        // 创建事件处理任务
        let context = self.context.clone();
        cx.spawn(async move |_cx| {
            while let Some(event) = rx.next().await {
                match event {
                    HttpHlsDownloaderEvent::Progress(kbs) => {
                        context.push_event(DownloadEvent::Progress {
                            bytes_downloaded: 0, // FFmpeg doesn't provide exact bytes
                            download_speed_kbps: kbs,
                            duration_ms: 0, // Could be calculated if needed
                        });
                    }
                    HttpHlsDownloaderEvent::Done => {
                        let file_size = std::fs::metadata(&config_clone.output_path)
                            .map(|m| m.len())
                            .unwrap_or(0);

                        context.push_event(DownloadEvent::Completed {
                            file_path: config_clone.output_path.clone(),
                            file_size,
                        });
                        break;
                    }
                    HttpHlsDownloaderEvent::Error(err) => {
                        context.push_event(DownloadEvent::Error {
                            error: err,
                            is_recoverable: true,
                        });
                    }
                }
            }
        })
        .detach();

        cx.background_executor()
            .spawn(async move {
                let process = Self::download_stream(&url, &config)
                    .context("无法创建 FFmpeg 上下文")
                    .unwrap();

                {
                    let mut lock = inner.lock().unwrap();
                    *lock = Some(process);
                }

                println!("开始下载HLS流: {url} {}", config.output_path);

                let mut lock = inner.lock().unwrap();
                if let Some(ref mut process) = *lock {
                    if let Ok(iter) = process.iter() {
                        for event in iter {
                            if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }

                            match event {
                                FfmpegEvent::Progress(progress) => {
                                    // println!("progress: {progress:?}");
                                    if let Err(e) = tx.try_send(HttpHlsDownloaderEvent::Progress(
                                        progress.bitrate_kbps,
                                    )) {
                                        eprintln!("发送进度事件失败: {e}");
                                    }
                                }
                                FfmpegEvent::Done => {
                                    if let Err(e) = tx.try_send(HttpHlsDownloaderEvent::Done) {
                                        eprintln!("发送完成事件失败: {e}");
                                    }
                                }
                                FfmpegEvent::LogEOF => {
                                    if let Err(e) = tx.try_send(HttpHlsDownloaderEvent::Done) {
                                        eprintln!("发送完成事件失败: {e}");
                                    }
                                }
                                FfmpegEvent::Log(level, msg) => {
                                    if level == ffmpeg_sidecar::event::LogLevel::Fatal {
                                        let _ = tx.try_send(HttpHlsDownloaderEvent::Error(msg));
                                    }
                                }
                                _ => {}
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

        self.emit_event(DownloadEvent::Paused);

        if let Ok(mut guard) = self.inner.lock() {
            if let Some(process) = guard.as_mut() {
                if let Err(e) = process.quit() {
                    process.wait().unwrap();
                    eprintln!("FFmpeg 进程退出失败: {e}");
                }
            }
        }

        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.status = DownloadStatus::Paused;

        self.emit_event(DownloadEvent::Paused);
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
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
