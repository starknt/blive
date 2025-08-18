use crate::core::downloader::context::DownloaderEvent;
use crate::core::downloader::{
    DownloadConfig, Downloader, DownloaderContext, DownloaderError, REFERER, USER_AGENT,
};
use crate::settings::StreamCodec;
use anyhow::Result;
use futures::channel::oneshot;
use gpui::AsyncApp;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[derive(Debug)]
pub struct HttpHlsDownloader {
    running: Arc<AtomicBool>,
    url: String,
    config: DownloadConfig,
    context: DownloaderContext,
    stop_rx: Option<oneshot::Receiver<()>>,
}

impl HttpHlsDownloader {
    pub fn new(url: String, config: DownloadConfig, context: DownloaderContext) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            url,
            config,
            context,
            stop_rx: None,
        }
    }

    #[cfg(feature = "ffmpeg")]
    fn download_stream(
        url: &str,
        config: &DownloadConfig,
    ) -> Result<ffmpeg_sidecar::child::FfmpegChild> {
        let mut cmd = ffmpeg_sidecar::command::FfmpegCommand::new();

        if config.overwrite {
            cmd.overwrite();
        } else {
            cmd.no_overwrite();
        }

        cmd.args(["-headers", format!("User-Agent: {USER_AGENT}").as_str()])
            .args(["-headers", format!("Referer: {REFERER}").as_str()])
            .arg("-i")
            .arg(url)
            .args(["-vf", "scale=1920:1080"])
            .args(["-c:a", "aac"])
            .args(["-bsf:a", "aac_adtstoasc"])
            .arg("-c:v")
            .arg(match config.codec {
                StreamCodec::AVC => "libx264",
                StreamCodec::HEVC => "hevc",
            })
            .arg(config.output_path.clone());

        let process = cmd.spawn().unwrap();

        Ok(process)
    }
}

impl Downloader for HttpHlsDownloader {
    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn set_running(&self, running: bool) {
        self.running
            .store(running, std::sync::atomic::Ordering::Relaxed);
    }

    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let url = self.url.clone();
        // 更新状态
        self.context.set_running(true);
        self.set_running(true);
        let config = self.config.clone();
        let output_path = config.output_path.clone();

        // 发送开始事件
        self.context.push_event(DownloaderEvent::Started {
            file_path: output_path.clone(),
        });

        let (stop_tx, stop_rx) = oneshot::channel();
        self.stop_rx = Some(stop_rx);

        let context = self.context.clone();
        let is_running = self.running.clone();
        let start_time = Instant::now();
        let mut bytes_downloaded = 0;

        #[cfg(feature = "ffmpeg")]
        cx.background_executor()
            .spawn(async move {
                let mut process = match Self::download_stream(&url, &config) {
                    Ok(p) => p,
                    Err(e) => {
                        context.push_event(DownloaderEvent::Error {
                            error: DownloaderError::StartupFailed {
                                command: format!("ffmpeg -i {url}"),
                                stderr: e.to_string(),
                            },
                        });
                        return;
                    }
                };

                match process.iter() {
                    Ok(iter) => {
                        for event in iter {
                            // 检查是否收到停止信号
                            if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                                process.quit().unwrap();
                                if let Err(e) = process.wait() {
                                    eprintln!("FFmpeg进程wait失败: {e}");
                                } else {
                                    println!("FFmpeg进程已成功清理");
                                }
                                context.push_event(DownloaderEvent::Completed {
                                    file_path: output_path.clone(),
                                    file_size: bytes_downloaded,
                                    duration: start_time.elapsed().as_secs_f64() as u64,
                                });
                                let _ = stop_tx.send(());
                                return;
                            }

                            match event {
                                ffmpeg_sidecar::event::FfmpegEvent::Progress(progress) => {
                                    bytes_downloaded += progress.size_kb as u64;
                                    context.push_event(DownloaderEvent::Progress {
                                        bytes_downloaded,
                                        download_speed_kbps: progress.bitrate_kbps,
                                        duration_ms: start_time.elapsed().as_millis() as u64,
                                    });
                                }
                                ffmpeg_sidecar::event::FfmpegEvent::Done => {
                                    context.push_event(DownloaderEvent::Completed {
                                        file_path: output_path.clone(),
                                        file_size: bytes_downloaded,
                                        duration: start_time.elapsed().as_secs_f64() as u64,
                                    });
                                }
                                ffmpeg_sidecar::event::FfmpegEvent::LogEOF => {
                                    context.push_event(DownloaderEvent::Completed {
                                        file_path: output_path.clone(),
                                        file_size: bytes_downloaded,
                                        duration: start_time.elapsed().as_secs_f64() as u64,
                                    });
                                }
                                ffmpeg_sidecar::event::FfmpegEvent::Log(level, message) => {
                                    match level {
                                        ffmpeg_sidecar::event::LogLevel::Fatal => {
                                            context.push_event(DownloaderEvent::Error {
                                                error: DownloaderError::FfmpegFatalError {
                                                    message,
                                                },
                                            });
                                        }
                                        ffmpeg_sidecar::event::LogLevel::Error => {
                                            // 根据错误消息智能分类
                                            if message.contains("Connection reset")
                                                || message.contains("timeout")
                                                || message.contains("No route to host")
                                                || message.contains("Connection refused")
                                            {
                                                context.push_event(DownloaderEvent::Error {
                                                    error:
                                                        DownloaderError::NetworkConnectionFailed {
                                                            message,
                                                        },
                                                });
                                            } else if message.contains("Protocol not found")
                                                || message.contains("Invalid data found")
                                                || message.contains("Decoder failed")
                                            {
                                                context.push_event(DownloaderEvent::Error {
                                                    error:
                                                        DownloaderError::NoSuitableStreamProtocol,
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
                    Err(e) => {
                        context.push_event(DownloaderEvent::Error {
                            error: DownloaderError::StartupFailed {
                                command: "".to_string(),
                                stderr: e.to_string(),
                            },
                        });
                    }
                }
            })
            .detach();

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.set_running(true);

        if let Some(stop_rx) = self.stop_rx.take() {
            match stop_rx.await {
                Ok(_) => {
                    println!("成功触发停止信号");
                    self.context.set_running(false);
                }
                Err(e) => {
                    eprintln!("停止信号发送失败: {e}");
                    self.context.set_running(false);
                }
            }
        }

        Ok(())
    }
}
