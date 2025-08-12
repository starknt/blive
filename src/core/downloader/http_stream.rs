use crate::core::downloader::{
    DownloadConfig, DownloadEvent, DownloadStatus, Downloader, DownloaderContext, DownloaderError,
    REFERER, USER_AGENT,
};
use crate::settings::{Strategy, StreamCodec};
use anyhow::{Context, Result};
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use futures::AsyncReadExt;
use futures::channel::oneshot;
use gpui::AsyncApp;
use gpui::http_client::{AsyncBody, Method, Request};
use std::io::Write;
use std::time::Instant;

pub struct HttpStreamDownloader {
    url: String,
    config: DownloadConfig,
    context: DownloaderContext,
    stop_rx: Option<oneshot::Receiver<()>>,
}

impl HttpStreamDownloader {
    pub fn new(url: String, config: DownloadConfig, context: DownloaderContext) -> Self {
        Self {
            url,
            config,
            context,
            stop_rx: None,
        }
    }

    fn download_stream(url: &str, config: &DownloadConfig) -> Result<FfmpegChild> {
        let mut cmd = FfmpegCommand::new();

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

        let process = cmd.spawn().context("无法启动FFmpeg进程")?;

        Ok(process)
    }
}

impl Downloader for HttpStreamDownloader {
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let url = self.url.clone();

        // 更新状态
        self.context.set_running(true);
        self.context.set_status(DownloadStatus::Downloading);

        let config = self.config.clone();
        let output_path = config.output_path.clone();

        // 发送开始事件
        self.context.push_event(DownloadEvent::Started {
            file_path: output_path.clone(),
        });

        let context = self.context.clone();
        let start_time = Instant::now();
        let mut bytes_downloaded = 0;
        let (stop_tx, stop_rx) = oneshot::channel();
        self.stop_rx = Some(stop_rx);

        match self.context.strategy {
            Strategy::LowCost => {
                cx.background_executor()
                    .spawn(async move {
                        let request = Request::builder()
                            .uri(url)
                            .header("User-Agent", USER_AGENT)
                            .header("Referer", REFERER)
                            .method(Method::GET)
                            .body(AsyncBody::empty())
                            .unwrap();

                        match context.client.send(request).await {
                            Ok(mut response) => {
                                if !response.status().is_success() {
                                    return context.push_event(DownloadEvent::Error {
                                        error: DownloaderError::NetworkError(format!(
                                            "HTTP请求失败: {}",
                                            response.status()
                                        )),
                                    });
                                }

                                let body = response.body_mut();
                                let mut buffer = [0; 8192];
                                let mut bytes_downloaded = 0u64;
                                let mut download_speed_kbps = 0f32;
                                let mut last_report_time = Instant::now();
                                let mut last_report_bytes = 0u64;

                                match std::fs::File::create(&config.output_path) {
                                    Ok(mut file) => {
                                        while let Ok(bytes_read) = body.read(&mut buffer).await {
                                            if !context.is_running() {
                                                context.push_event(DownloadEvent::Completed {
                                                    file_path: output_path.clone(),
                                                    file_size: bytes_downloaded,
                                                    duration: start_time.elapsed().as_secs_f64()
                                                        as u64,
                                                });
                                                let _ = stop_tx.send(());
                                                return;
                                            }

                                            if bytes_read == 0 {
                                                context.push_event(DownloadEvent::Completed {
                                                    file_path: config.output_path,
                                                    file_size: bytes_downloaded,
                                                    duration: start_time.elapsed().as_secs_f64()
                                                        as u64,
                                                });
                                                break; // EOF
                                            }

                                            match file.write_all(&buffer[..bytes_read]) {
                                                Ok(_) => {
                                                    bytes_downloaded += bytes_read as u64;
                                                    let duration_ms =
                                                        start_time.elapsed().as_millis() as u64;

                                                    // 计算下载速度（KBps）
                                                    let now = Instant::now();
                                                    let elapsed = now
                                                        .duration_since(last_report_time)
                                                        .as_secs_f64();
                                                    if elapsed > 1.0 {
                                                        let bytes_delta =
                                                            bytes_downloaded - last_report_bytes;
                                                        download_speed_kbps = ((bytes_delta as f64)
                                                            / 1024.0
                                                            / elapsed)
                                                            as f32;
                                                        last_report_time = now;
                                                        last_report_bytes = bytes_downloaded;
                                                    }

                                                    context.push_event(DownloadEvent::Progress {
                                                        bytes_downloaded,
                                                        download_speed_kbps,
                                                        duration_ms,
                                                    });
                                                }
                                                Err(e) => {
                                                    context.push_event(DownloadEvent::Error {
                                                        error: DownloaderError::FileSystemError(
                                                            e.to_string(),
                                                        ),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("无法创建输出文件: {e}");
                                        context.push_event(DownloadEvent::Error {
                                            error: DownloaderError::FileCreationFailed {
                                                path: config.output_path,
                                                reason: e.to_string(),
                                            },
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                context.push_event(DownloadEvent::Error {
                                    error: DownloaderError::NetworkError(format!(
                                        "HTTP请求失败: {e}"
                                    )),
                                });
                            }
                        }
                    })
                    .detach();
            }
            Strategy::PriorityConfig => {
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
                                        file_size: bytes_downloaded,
                                        duration: start_time.elapsed().as_secs_f64() as u64,
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
                                        context.push_event(DownloadEvent::Completed {
                                            file_path: output_path.clone(),
                                            file_size: bytes_downloaded,
                                            duration: start_time.elapsed().as_secs_f64() as u64,
                                        });
                                    }
                                    FfmpegEvent::LogEOF => {
                                        context.push_event(DownloadEvent::Completed {
                                            file_path: output_path.clone(),
                                            file_size: bytes_downloaded,
                                            duration: start_time.elapsed().as_secs_f64() as u64,
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
                                                        error: DownloaderError::network_connection_failed(msg),
                                                    });
                                                } else if msg.contains("Protocol not found")
                                                    || msg.contains("Invalid data found")
                                                    || msg.contains("Decoder failed")
                                                {
                                                    context.push_event(DownloadEvent::Error {
                                                        error:
                                                            DownloaderError::StreamEncodingError {
                                                                codec: "unknown".to_string(),
                                                                details: msg,
                                                            },
                                                    });
                                                } else {
                                                    #[cfg(debug_assertions)]
                                                    context.push_event(DownloadEvent::Error {
                                                        error:
                                                            DownloaderError::FfmpegRuntimeError {
                                                                error_type: "Error".to_string(),
                                                                message: msg,
                                                            },
                                                    });
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
            }
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.context.set_running(false);
        self.context.set_status(DownloadStatus::NotStarted);

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
        self.context.get_status()
    }
}
