use crate::core::downloader::{
    DownloadConfig, DownloadEvent, DownloadStats, DownloadStatus, Downloader, DownloaderContext,
    DownloaderError,
};
use crate::settings::StreamCodec;
use anyhow::{Context, Result};
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use futures::channel::oneshot;
use gpui::AsyncApp;
use std::time::Instant;

pub struct HttpStreamDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    stats: DownloadStats,
    start_time: Option<Instant>,
    context: DownloaderContext,
    stop_rx: Option<oneshot::Receiver<()>>,
}

impl HttpStreamDownloader {
    pub fn new(url: String, config: DownloadConfig, context: DownloaderContext) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
            stats: DownloadStats::default(),
            start_time: None,
            context,
            stop_rx: None,
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
                StreamCodec::HEVC => "copy",
            })
            .arg(config.output_path.clone());

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

        let context = self.context.clone();
        let start_time = Instant::now();
        let mut bytes_downloaded = 0;
        let (stop_tx, stop_rx) = oneshot::channel();
        self.stop_rx = Some(stop_rx);

        cx.background_executor()
            .spawn(async move {
                let mut process = match Self::download_stream(&url, &config) {
                    Ok(p) => p,
                    Err(e) => {
                        context.push_event(DownloadEvent::Error {
                            error: DownloaderError::StartupFailed {
                                command: format!("ffmpeg -i {url}"),
                                stderr: e.to_string(),
                            },
                        });
                        return;
                    }
                };

                if let Ok(iter) = process.iter() {
                    for event in iter {
                        // 检查是否收到停止信号
                        if !context.is_running() {
                            process.quit().unwrap();
                            if let Err(e) = process.wait() {
                                eprintln!("FFmpeg进程wait失败: {e}");
                            } else {
                                println!("FFmpeg进程已成功清理");
                            }
                            context.push_event(DownloadEvent::Completed {
                                file_path: output_path.clone(),
                                file_size: 0,
                            });
                            let _ = stop_tx.send(());
                            return;
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
                                let file_size = std::fs::metadata(&output_path)
                                    .map(|m| m.len())
                                    .unwrap_or(bytes_downloaded);

                                context.push_event(DownloadEvent::Completed {
                                    file_path: output_path.clone(),
                                    file_size,
                                });
                            }
                            FfmpegEvent::LogEOF => {
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
                                                error: DownloaderError::network_connection_failed(
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
                                            // #[cfg(debug_assertions)]
                                            // context.push_event(DownloadEvent::Error {
                                            //     error: DownloaderError::FfmpegRuntimeError {
                                            //         error_type: "Error".to_string(),
                                            //         message: msg,
                                            //     },
                                            // });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
            })
            .detach();

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.status = DownloadStatus::NotStarted;
        self.context.set_running(false);

        if let Some(stop_rx) = self.stop_rx.take() {
            match stop_rx.await {
                Ok(_) => {
                    println!("成功触发停止信号");
                }
                Err(e) => {
                    eprintln!("停止信号发送失败: {e}");
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
