use crate::core::downloader::{
    DownloadConfig, DownloadEvent, DownloadStats, DownloadStatus, Downloader, DownloaderContext,
    DownloaderError,
};
use crate::settings::StreamCodec;
use anyhow::{Context, Result};
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use gpui::AsyncApp;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct HttpHlsDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    inner: Arc<Mutex<Option<FfmpegChild>>>,
    stats: DownloadStats,
    start_time: Option<Instant>,
    context: DownloaderContext,
}

impl HttpHlsDownloader {
    pub fn new(url: String, config: DownloadConfig, context: DownloaderContext) -> Self {
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

        if config.overwrite {
            cmd.overwrite();
        } else {
            cmd.no_overwrite();
        }

        cmd.arg("-headers")
            .arg(user_agent_header)
            .arg("-headers")
            .arg(referer_header)
            .arg("-i")
            .arg(url)
            .args(["-vf", "scale=1920:1080"])
            .args(["-c:a", "aac"])
            .args(["-bsf:a", "aac_adtstoasc"])
            .arg("-c:v")
            .arg(match config.codec {
                StreamCodec::AVC => "copy",
                StreamCodec::HEVC => "hevc",
            })
            .arg(config.output_path.clone());

        let process = cmd.spawn().unwrap();

        Ok(process)
    }
}

impl Downloader for HttpHlsDownloader {
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
                let process = Self::download_stream(&url, &config)
                    .context("无法创建 FFmpeg 上下文")
                    .unwrap();

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
                                    bytes_downloaded += progress.size_kb as u64;
                                    context.push_event(DownloadEvent::Progress {
                                        bytes_downloaded,
                                        download_speed_kbps: progress.bitrate_kbps,
                                        duration_ms: start_time.elapsed().as_millis() as u64,
                                    });
                                }
                                FfmpegEvent::Done => {
                                    context.push_event(DownloadEvent::Completed {
                                        file_path: output_path.clone(),
                                        file_size: 0,
                                    });
                                }
                                FfmpegEvent::LogEOF => {
                                    context.push_event(DownloadEvent::Completed {
                                        file_path: output_path.clone(),
                                        file_size: 0,
                                    });
                                }
                                FfmpegEvent::Log(level, msg) => {
                                    match level {
                                        ffmpeg_sidecar::event::LogLevel::Fatal => {
                                            context.push_event(DownloadEvent::Error {
                                                error: DownloaderError::FfmpegRuntimeError {
                                                    error_type: "Fatal".to_string(),
                                                    message: msg,
                                                },
                                            });
                                        }
                                        ffmpeg_sidecar::event::LogLevel::Error => {
                                            // 根据错误消息智能分类
                                            if msg.contains("Connection reset")
                                                || msg.contains("timeout")
                                                || msg.contains("No route to host")
                                                || msg.contains("Connection refused")
                                            {
                                                context.push_event(DownloadEvent::Error {
                                                    error:
                                                        DownloaderError::network_connection_failed(
                                                            msg, 0,
                                                        ),
                                                });
                                            } else if msg.contains("Protocol not found")
                                                || msg.contains("Invalid data found")
                                                || msg.contains("Decoder failed")
                                            {
                                                context.push_event(DownloadEvent::Error {
                                                    error: DownloaderError::StreamEncodingError {
                                                        codec: "unknown".to_string(),
                                                        details: msg,
                                                    },
                                                });
                                            } else {
                                                #[cfg(debug_assertions)]
                                                context.push_event(DownloadEvent::Error {
                                                    error: DownloaderError::FfmpegRuntimeError {
                                                        error_type: "Error".to_string(),
                                                        message: msg,
                                                    },
                                                });
                                            }
                                        }
                                        _ => {
                                            // 其他日志级别暂时忽略
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
                    process.wait().unwrap();
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
