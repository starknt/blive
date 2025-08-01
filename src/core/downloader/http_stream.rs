use crate::core::downloader::{
    DownloadConfig, DownloadEvent, DownloadStats, DownloadStatus, Downloader, DownloaderContext,
};
use crate::settings::StreamCodec;
use anyhow::{Context, Result};
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use gpui::AsyncApp;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct HttpStreamDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    inner: Arc<Mutex<Option<FfmpegChild>>>,
    stats: DownloadStats,
    start_time: Option<Instant>,
    context: DownloaderContext,
}

impl HttpStreamDownloader {
    pub fn new(
        url: String,
        config: DownloadConfig,
        _client: crate::core::http_client::HttpClient, // 保留参数以兼容接口，但不使用
        context: DownloaderContext,
    ) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
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
            .arg("-reconnect")
            .arg("1")
            .arg("-reconnect_streamed")
            .arg("1")
            .arg("-reconnect_delay_max")
            .arg("30")
            .arg("-i")
            .arg(url)
            .arg("-c")
            .arg("copy")
            .arg("-c:v")
            .arg(match config.codec {
                StreamCodec::AVC => "copy",
                StreamCodec::HEVC => "copy", // 对于HTTP流，通常也是copy
            })
            .arg("-c:a")
            .arg("copy")
            .arg(config.output_path.clone());

        if config.overwrite {
            cmd.overwrite();
        } else {
            cmd.no_overwrite();
        }

        let process = cmd.spawn().context("无法启动FFmpeg进程")?;

        Ok(process)
    }
}

impl Downloader for HttpStreamDownloader {
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let url = self.url.clone();

        // 更新状态
        self.status = DownloadStatus::Downloading;
        self.start_time = Some(Instant::now());
        self.context.set_running(true);

        let config = self.config.clone();
        let output_path = config.output_path.clone();

        // 发送开始事件
        self.emit_event(DownloadEvent::Started {
            file_path: output_path.clone(),
        });

        let inner = self.inner.clone();
        let context = self.context.clone();
        let start_time = Instant::now();
        let mut bytes_downloaded = 0;

        cx.background_executor()
            .spawn(async move {
                let process = match Self::download_stream(&url, &config) {
                    Ok(p) => p,
                    Err(e) => {
                        context.push_event(DownloadEvent::Error {
                            error: format!("启动FFmpeg失败: {e}"),
                            is_recoverable: false,
                        });
                        return;
                    }
                };

                {
                    let mut lock = inner.lock().unwrap();
                    *lock = Some(process);
                }

                let mut lock = inner.lock().unwrap();
                if let Some(ref mut process) = *lock {
                    if let Ok(iter) = process.iter() {
                        for event in iter {
                            if !context.is_running() {
                                break;
                            }

                            match event {
                                FfmpegEvent::Progress(progress) => {
                                    bytes_downloaded = progress.size_kb as u64 * 1024; // 转换为字节
                                    let duration_ms = start_time.elapsed().as_millis() as u64;

                                    context.push_event(DownloadEvent::Progress {
                                        bytes_downloaded,
                                        download_speed_kbps: progress.bitrate_kbps,
                                        duration_ms,
                                    });
                                }
                                FfmpegEvent::Done => {
                                    // 获取文件大小
                                    let file_size = std::fs::metadata(&output_path)
                                        .map(|m| m.len())
                                        .unwrap_or(bytes_downloaded);

                                    context.push_event(DownloadEvent::Completed {
                                        file_path: output_path.clone(),
                                        file_size,
                                    });
                                }
                                FfmpegEvent::LogEOF => {
                                    // 流结束，获取文件大小
                                    let file_size = std::fs::metadata(&output_path)
                                        .map(|m| m.len())
                                        .unwrap_or(bytes_downloaded);

                                    context.push_event(DownloadEvent::Completed {
                                        file_path: output_path.clone(),
                                        file_size,
                                    });
                                }
                                FfmpegEvent::Log(level, msg) => {
                                    match level {
                                        ffmpeg_sidecar::event::LogLevel::Fatal => {
                                            context.push_event(DownloadEvent::Error {
                                                error: format!("FFmpeg致命错误: {msg}"),
                                                is_recoverable: true,
                                            });
                                        }
                                        ffmpeg_sidecar::event::LogLevel::Error => {
                                            // 某些错误可能是可恢复的
                                            if msg.contains("Connection reset")
                                                || msg.contains("timeout")
                                                || msg.contains("No route to host")
                                            {
                                                context.push_event(DownloadEvent::Error {
                                                    error: format!("网络错误: {msg}"),
                                                    is_recoverable: true,
                                                });
                                            }
                                        }
                                        _ => {
                                            // 其他日志级别暂时忽略
                                            #[cfg(debug_assertions)]
                                            eprintln!("FFmpeg {level:?}: {msg}");
                                        }
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
        self.context.set_running(false);
        self.status = DownloadStatus::NotStarted;

        if let Ok(mut guard) = self.inner.lock() {
            if let Some(process) = guard.as_mut() {
                if let Err(e) = process.quit() {
                    // 如果优雅退出失败，强制终止
                    let _ = process.wait();
                    eprintln!("FFmpeg 进程退出失败: {e}");
                }
            }
        }

        Ok(())
    }

    fn status(&self) -> DownloadStatus {
        self.status.clone()
    }

    fn stats(&self) -> DownloadStats {
        self.stats.clone()
    }
}
