use crate::components::{RoomCard, RoomCardStatus};
use crate::core::downloader::{DownloadConfig, DownloadStatus, Downloader};
use crate::settings::StreamCodec;
use anyhow::{Context, Result};
use ez_ffmpeg::core::scheduler::ffmpeg_scheduler::Initialization;
use ez_ffmpeg::{FfmpegContext, FfmpegScheduler, Input, Output, error};
use gpui::{AsyncApp, WeakEntity};
use std::sync::Arc;

pub struct HttpHlsDownloader {
    url: String,
    config: DownloadConfig,
    status: DownloadStatus,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    entity: WeakEntity<RoomCard>,
}

impl HttpHlsDownloader {
    pub fn new(url: String, config: DownloadConfig, entity: WeakEntity<RoomCard>) -> Self {
        Self {
            url,
            config,
            status: DownloadStatus::NotStarted,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            entity,
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
            })
            .set_format("matroska");

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

        // 确保输出目录存在
        self.ensure_output_directory()?;

        // 更新状态
        self.status = DownloadStatus::Downloading;
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        match Self::download_stream(&url, &config).context("无法创建 FFmpeg 上下文") {
            Ok(scheduler) => match scheduler.start() {
                Ok(scheduler) => {
                    cx.spawn(async move |_cx| match scheduler.await {
                        Ok(_) => {}
                        Err(e) => match e {
                            error::Error::AllocFrame(f) => {
                                println!("FFmpeg 上下文错误: {f}");
                            }
                            error::Error::AllocOutputContext(c) => {
                                println!("FFmpeg 上下文错误: {c}");
                            }
                            error::Error::AllocPacket(p) => {
                                println!("FFmpeg 上下文错误: {p}");
                            }
                            error::Error::Decoder(e) => {
                                println!("FFmpeg 上下文错误: {e}");
                            }
                            error::Error::Demuxing(e) => {
                                println!("FFmpeg 上下文错误: {e}");
                            }
                            error::Error::Decoding(e) => {
                                println!("FFmpeg 上下文错误: {e}");
                            }
                            error::Error::EOF => {
                                println!("FFmpeg 上下文错误: 文件结束");
                            }
                            error::Error::Encoding(e) => {
                                println!("FFmpeg 上下文错误: {e}");
                            }
                            error::Error::Bug => {
                                println!("FFmpeg 上下文错误: 未知错误");
                            }
                            error::Error::NotStarted => {
                                println!("FFmpeg 上下文错误: 未启动");
                            }
                            error::Error::Url(u) => {
                                println!("FFmpeg 上下文错误: {u}");
                            }
                            error::Error::Exit => {
                                println!("FFmpeg 上下文错误: 退出");
                            }
                            error::Error::OpenInputStream(i) => {
                                println!("FFmpeg 上下文错误: {i}");
                            }
                            error::Error::FindStream(f) => {
                                println!("FFmpeg 上下文错误: {f}");
                            }
                            error::Error::FilterGraphParse(f) => {
                                println!("FFmpeg 上下文错误: {f}");
                            }
                            error::Error::FilterDescUtf8 => {
                                println!("FFmpeg 上下文错误: 过滤器描述 UTF-8");
                            }
                            error::Error::FileSameAsInput(e) => {
                                println!("FFmpeg 上下文错误: {e}");
                            }
                            error::Error::FilterNameUtf8 => {
                                println!("FFmpeg 上下文错误: 过滤器名称 UTF-8");
                            }
                            error::Error::FilterZeroOutputs => {
                                println!("FFmpeg 上下文错误: 过滤器零输出");
                            }
                            error::Error::ParseInteger => {
                                println!("FFmpeg 上下文错误: 解析整数");
                            }
                            error::Error::OpenDecoder(d) => {
                                println!("FFmpeg 上下文错误: {d}");
                            }
                            error::Error::OpenEncoder(e) => {
                                println!("FFmpeg 上下文错误: {e}");
                            }
                            error::Error::Muxing(m) => {
                                println!("FFmpeg 上下文错误: {m}");
                            }
                            error::Error::OpenOutput(open_output_error) => {
                                println!("FFmpeg 上下文错误: {open_output_error}");
                            }
                            error::Error::FindDevices(find_devices_error) => {
                                println!("FFmpeg 上下文错误: {find_devices_error}");
                            }
                            error::Error::FilterGraph(filter_graph_operation_error) => {
                                println!("FFmpeg 上下文错误: {filter_graph_operation_error}");
                            }
                            error::Error::FrameFilterInit(_) => {
                                println!("FFmpeg 上下文错误: 帧过滤器初始化");
                            }
                            error::Error::FrameFilterProcess(_) => {
                                println!("FFmpeg 上下文错误: 帧过滤器处理");
                            }
                            error::Error::FrameFilterRequest(_) => {
                                println!("FFmpeg 上下文错误: 帧过滤器请求");
                            }
                            error::Error::FrameFilterTypeNoMatched(_, _) => {
                                println!("FFmpeg 上下文错误: 帧过滤器类型不匹配");
                            }
                            error::Error::FrameFilterStreamTypeNoMatched(_, _, _) => {
                                println!("FFmpeg 上下文错误: 帧过滤器流类型不匹配");
                            }
                            error::Error::FrameFilterThreadExited => {
                                println!("FFmpeg 上下文错误: 帧过滤器线程退出");
                            }
                            error::Error::IO(error) => {
                                println!("FFmpeg 上下文错误: {error}");
                            }
                            error::Error::FrameFilterDstFinished => {
                                println!("FFmpeg 上下文错误: 帧过滤器目标完成");
                            }
                            error::Error::FrameFilterSendOOM => {
                                println!("FFmpeg 上下文错误: 帧过滤器发送 OOM");
                            }
                        },
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
            },
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

        Ok(())
    }

    fn status(&self) -> DownloadStatus {
        self.status.clone()
    }
}
