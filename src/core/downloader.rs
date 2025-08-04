pub mod context;
pub mod error;
pub mod http_hls;
pub mod http_stream;
pub mod stats;
pub mod template;
pub mod utils;

use crate::components::{RoomCard, RoomCardStatus};
use crate::core::downloader::error::DownloaderError;
use crate::core::downloader::template::DownloaderFilenameTemplate;
use crate::core::downloader::{http_hls::HttpHlsDownloader, http_stream::HttpStreamDownloader};
use crate::core::http_client::HttpClient;
use crate::core::http_client::room::LiveRoomInfoData;
use crate::core::http_client::stream::{LiveRoomStreamUrl, PlayStream};
use crate::core::http_client::user::LiveUserInfo;
use crate::settings::{DEFAULT_RECORD_NAME, LiveProtocol, Quality, StreamCodec, VideoContainer};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
pub use context::{DownloadConfig, DownloaderContext};
use gpui::{AsyncApp, WeakEntity};
use rand::Rng;
pub use stats::DownloadStats;
use std::sync::Mutex;
use std::time::Duration;

pub const REFERER: &str = "https://live.bilibili.com/";
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    /// 下载开始
    Started { file_path: String },
    /// 进度更新
    Progress {
        bytes_downloaded: u64,
        download_speed_kbps: f32,
        duration_ms: u64,
    },
    /// 下载完成
    Completed { file_path: String, file_size: u64 },
    /// 下载错误
    Error { error: DownloaderError },
    /// 网络重连中
    Reconnecting { attempt: u32, delay_secs: u64 },
}

pub trait Downloader {
    /// 开始下载
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()>;

    /// 停止下载
    fn stop(&mut self) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 获取下载状态
    fn status(&self) -> DownloadStatus;
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    /// 未开始
    NotStarted,
    /// 下载中
    Downloading,
    /// 已完成
    Completed,
    /// 重连中
    Reconnecting,
    /// 错误
    Error(String),
}

pub enum DownloaderType {
    HttpStream(HttpStreamDownloader),
    HttpHls(HttpHlsDownloader),
}

pub struct BLiveDownloader {
    context: DownloaderContext,
    downloader: Mutex<Option<DownloaderType>>,
    // 网络重连相关字段
    max_reconnect_attempts: Mutex<u32>,
    reconnect_delay: Mutex<Duration>,
    is_auto_reconnect: Mutex<bool>,
}

impl BLiveDownloader {
    async fn start_download(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        // 获取流信息
        let stream_info = self.get_stream_info().await?;

        // 解析下载URL和选择下载器类型
        let (url, downloader_type) = self.parse_stream_url(&stream_info)?;

        // 生成文件名
        let filename = self.generate_filename()?;

        // 获取文件扩展名
        let ext = self.context.format.ext();

        // 处理文件路径冲突
        let file_path = self.resolve_file_path(record_dir, &filename, ext)?;

        // 根据下载器类型创建具体的下载器
        let mut final_downloader = match downloader_type {
            DownloaderType::HttpStream(_) => {
                let config = DownloadConfig {
                    output_path: file_path.clone(),
                    overwrite: false,
                    timeout: 30,
                    retry_count: 3,
                    codec: self.context.codec,
                    format: self.context.format,
                    quality: self.context.quality,
                };
                let downloader = HttpStreamDownloader::new(url, config, self.context.clone());

                DownloaderType::HttpStream(downloader)
            }
            DownloaderType::HttpHls(_) => {
                let config = DownloadConfig {
                    output_path: file_path.clone(),
                    overwrite: false,
                    timeout: 30,
                    retry_count: 3,
                    codec: self.context.codec,
                    format: self.context.format,
                    quality: self.context.quality,
                };
                let downloader = HttpHlsDownloader::new(url, config, self.context.clone());

                DownloaderType::HttpHls(downloader)
            }
        };

        match &mut final_downloader {
            DownloaderType::HttpStream(downloader) => match downloader.start(cx) {
                Ok(_) => {
                    // 设置运行状态
                    self.context.set_running(true);

                    // 启动事件处理器
                    self.context.start_event_processor(cx);
                }
                Err(e) => {
                    return Err(e);
                }
            },
            DownloaderType::HttpHls(downloader) => match downloader.start(cx) {
                Ok(_) => {
                    // 设置运行状态
                    self.context.set_running(true);

                    // 启动事件处理器
                    self.context.start_event_processor(cx);
                }
                Err(e) => {
                    return Err(e);
                }
            },
        }

        self.downloader.lock().unwrap().replace(final_downloader);

        Ok(())
    }

    /// 带重连的下载方法
    pub async fn start(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        let mut retry_count = 0;

        loop {
            match self.start_download(cx, record_dir).await {
                Ok(_) => {
                    // 下载成功启动，重置重连计数
                    self.context.update_stats(|stats| {
                        stats.reconnect_count = 0;
                    });

                    // 更新UI状态为录制中
                    self.update_card_status(cx, RoomCardStatus::Recording(0.0));

                    // 下载成功启动，现在监控下载状态
                    if self.is_auto_reconnect() {
                        // 启动状态监控，处理自动重连和状态管理
                        self.monitor_download_status(cx, record_dir).await?;
                    }
                    return Ok(());
                }
                Err(e) if Self::is_network_error(&e) => {
                    retry_count += 1;
                    self.context.update_stats(|stats| {
                        stats.reconnect_count = retry_count;
                    });

                    let delay = self.calculate_backoff_delay(retry_count);

                    eprintln!("网络异常，正在尝试重连 (第{retry_count}次，等待{delay:?}): {e}");

                    // 更新UI状态显示重连信息
                    self.update_card_status(
                        cx,
                        RoomCardStatus::Error(format!(
                            "网络中断，第{retry_count}次重连 ({delay_secs}秒后)",
                            delay_secs = delay.as_secs()
                        )),
                    );

                    // 发送重连事件
                    self.context.push_event(DownloadEvent::Reconnecting {
                        attempt: retry_count,
                        delay_secs: delay.as_secs(),
                    });

                    cx.background_executor().timer(delay).await;
                    continue;
                }
                Err(e) => {
                    // 非网络错误，直接返回
                    eprintln!("非网络错误，停止重连: {e}");

                    // 更新UI状态显示错误
                    self.update_card_status(cx, RoomCardStatus::Error(format!("录制失败: {e}")));

                    // 发送错误事件
                    self.context.push_event(DownloadEvent::Error {
                        error: DownloaderError::InvalidRecordingConfig {
                            field: "stream_url".to_string(),
                            value: "unavailable".to_string(),
                            reason: format!("非网络错误: {e}"),
                        },
                    });

                    return Err(e);
                }
            }
        }
    }

    pub async fn stop(&self) {
        // 设置停止状态
        self.context.set_running(false);

        {
            let mut downloader_guard = self.downloader.lock().unwrap();
            if let Some(ref mut downloader) = downloader_guard.as_mut() {
                match downloader {
                    DownloaderType::HttpStream(downloader) => {
                        let _ = downloader.stop().await;
                    }
                    DownloaderType::HttpHls(downloader) => {
                        let _ = downloader.stop().await;
                    }
                }
            }
        }
    }

    /// 监控下载状态，根据事件处理重连或停止
    pub async fn monitor_download_status(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 5;

        while self.context.is_running() {
            // 检查下载器状态
            let status = {
                if let Ok(downloader_guard) = self.downloader.lock() {
                    if let Some(ref downloader) = downloader_guard.as_ref() {
                        match downloader {
                            DownloaderType::HttpStream(downloader) => downloader.status(),
                            DownloaderType::HttpHls(downloader) => downloader.status(),
                        }
                    } else {
                        DownloadStatus::NotStarted
                    }
                } else {
                    DownloadStatus::Error("无法获取下载器锁".to_string())
                }
            };

            match status {
                DownloadStatus::Error(error) => {
                    consecutive_errors += 1;

                    // 判断是否为网络错误
                    let is_network_error = Self::is_network_error(&anyhow::anyhow!("{}", error));

                    if is_network_error
                        && consecutive_errors <= MAX_CONSECUTIVE_ERRORS
                        && self.is_auto_reconnect()
                    {
                        // 发送错误事件（可恢复）
                        self.context.push_event(DownloadEvent::Error {
                            error: DownloaderError::NetworkError(error.clone()),
                        });

                        // 停止当前下载器
                        self.stop().await;

                        // 计算退避延迟
                        let delay = self.calculate_backoff_delay(consecutive_errors);

                        // 发送重连事件
                        self.context.push_event(DownloadEvent::Reconnecting {
                            attempt: consecutive_errors,
                            delay_secs: delay.as_secs(),
                        });

                        // 等待后重新启动下载
                        cx.background_executor().timer(delay).await;

                        match self.start_download(cx, record_dir).await {
                            Ok(_) => {
                                consecutive_errors = 0; // 重置错误计数
                                eprintln!("✅ 重连成功");
                            }
                            Err(e) => {
                                eprintln!("❌ 重连失败: {e}");
                            }
                        }
                    } else {
                        // 不可恢复错误或超过最大重试次数
                        self.context.push_event(DownloadEvent::Error {
                            error: DownloaderError::NetworkError(format!(
                                "连续错误超过{MAX_CONSECUTIVE_ERRORS}次，停止重连: {error}"
                            )),
                        });

                        self.stop().await;
                        break;
                    }
                }
                DownloadStatus::Completed => {
                    // 下载完成
                    if let Some(stats) = self.get_download_stats() {
                        self.context.push_event(DownloadEvent::Completed {
                            file_path: "".to_string(), // 具体路径由下载器提供
                            file_size: stats.bytes_downloaded,
                        });
                    }
                    break;
                }
                DownloadStatus::Downloading => {
                    consecutive_errors = 0; // 重置错误计数

                    // 更新进度
                    if let Some(stats) = self.get_download_stats() {
                        self.context.push_event(DownloadEvent::Progress {
                            bytes_downloaded: stats.bytes_downloaded,
                            download_speed_kbps: stats.download_speed_kbps,
                            duration_ms: stats.duration_ms,
                        });
                    }
                }
                DownloadStatus::Reconnecting => {
                    // 下载器内部正在重连，保持等待
                }
                DownloadStatus::NotStarted => {
                    // 下载器未启动，可能需要重新启动
                    eprintln!("⚠️  下载器未启动，尝试重新启动");
                    match self.start_download(cx, record_dir).await {
                        Ok(_) => {
                            eprintln!("✅ 重新启动成功");
                        }
                        Err(e) => {
                            eprintln!("❌ 重新启动失败: {e}");
                            consecutive_errors += 1;
                        }
                    }
                }
            }

            // 等待一段时间后再次检查
            cx.background_executor().timer(Duration::from_secs(2)).await;
        }

        Ok(())
    }
}

impl BLiveDownloader {
    pub fn new(
        room_info: LiveRoomInfoData,
        user_info: LiveUserInfo,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
        client: HttpClient,
        entity: WeakEntity<RoomCard>,
    ) -> Self {
        let context: DownloaderContext =
            DownloaderContext::new(entity, client, room_info, user_info, quality, format, codec);
        Self {
            context,
            downloader: Mutex::new(None),
            max_reconnect_attempts: Mutex::new(u32::MAX), // 无限重试
            reconnect_delay: Mutex::new(Duration::from_secs(1)), // 初始延迟1秒
            is_auto_reconnect: Mutex::new(true),          // 是否启用自动重连
        }
    }

    fn update_card_status(&self, cx: &mut AsyncApp, status: RoomCardStatus) {
        self.context.update_card_status(cx, status);
    }

    fn is_auto_reconnect(&self) -> bool {
        *self.is_auto_reconnect.lock().unwrap()
    }

    /// 检查是否为网络相关错误
    fn is_network_error(error: &anyhow::Error) -> bool {
        let error_str = error.to_string().to_lowercase();

        // 检查常见的网络错误关键词
        error_str.contains("network")
            || error_str.contains("connection")
            || error_str.contains("timeout")
            || error_str.contains("dns")
            || error_str.contains("socket")
            || error_str.contains("unreachable")
            || error_str.contains("reset")
            || error_str.contains("refused")
            || error_str.contains("无法连接")
            || error_str.contains("连接被重置")
            || error_str.contains("连接超时")
            || error_str.contains("网络")
            || error_str.contains("请求失败")
            || error_str.contains("无法读取响应体")
    }

    /// 获取下载统计信息
    fn get_download_stats(&self) -> Option<DownloadStats> {
        Some(self.context.get_stats())
    }

    /// 设置重连参数
    pub fn set_reconnect_config(
        &mut self,
        max_attempts: u32,
        initial_delay: Duration,
        auto_reconnect: bool,
    ) {
        let mut max_reconnect_attempts = self.max_reconnect_attempts.lock().unwrap();
        let mut reconnect_delay = self.reconnect_delay.lock().unwrap();
        let mut is_auto_reconnect = self.is_auto_reconnect.lock().unwrap();

        *max_reconnect_attempts = max_attempts;
        *reconnect_delay = initial_delay;
        *is_auto_reconnect = auto_reconnect;
    }

    /// 计算指数退避延迟，最大等待时间30分钟
    fn calculate_backoff_delay(&self, retry_count: u32) -> Duration {
        const MAX_DELAY: Duration = Duration::from_secs(30 * 60); // 30分钟

        let reconnect_delay = *self.reconnect_delay.lock().unwrap();

        // 指数退避：1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 1800(30分钟)
        let exponential_delay = reconnect_delay * (2_u32.pow(retry_count.min(10)));

        // 限制最大延迟为30分钟
        if exponential_delay > MAX_DELAY {
            MAX_DELAY
        } else {
            exponential_delay
        }
    }

    /// 获取直播流信息
    async fn get_stream_info(&self) -> Result<LiveRoomStreamUrl> {
        let mut retry_count = 0;

        loop {
            match self
                .context
                .client
                .get_live_room_stream_url(
                    self.context.room_info.room_id,
                    self.context.quality.to_quality(),
                )
                .await
            {
                Ok(stream_info) => return Ok(stream_info),
                Err(e) => {
                    retry_count += 1;
                    let delay = self.calculate_backoff_delay(retry_count);

                    eprintln!(
                        "获取直播流地址失败，正在重试 (第{retry_count}次，等待{delay:?}): {e}"
                    );

                    // 使用指数退避重试，无限重试
                    std::thread::sleep(delay);
                }
            }
        }
    }

    fn parse_stream_url(
        &self,
        stream_info: &LiveRoomStreamUrl,
    ) -> Result<(String, DownloaderType)> {
        let playurl_info = stream_info
            .playurl_info
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("未找到播放信息"))?;

        // 优先尝试http_hls协议
        if let Some(stream) = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == LiveProtocol::default())
        {
            return self.parse_hls_stream(stream);
        }

        // 如果没有http_hls，尝试http_stream协议
        if let Some(stream) = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == LiveProtocol::HttpStream)
        {
            return self.parse_http_stream(stream);
        }

        anyhow::bail!("未找到合适的直播流协议");
    }

    fn parse_http_stream(&self, stream: &PlayStream) -> Result<(String, DownloaderType)> {
        if stream.format.is_empty() {
            anyhow::bail!("未找到合适的直播流");
        }

        // 优先选择配置中的格式
        let format_stream = stream
            .format
            .iter()
            .find(|format| format.format_name == self.context.format)
            .or_else(|| stream.format.first())
            .ok_or_else(|| anyhow::anyhow!("未找到合适的视频格式"))?;

        if format_stream.codec.is_empty() {
            anyhow::bail!("未找到合适的视频编码");
        }

        // 优先按照设置选择编码格式
        let codec = format_stream
            .codec
            .iter()
            .find(|codec| codec.codec_name == self.context.codec)
            .unwrap_or_else(|| format_stream.codec.first().unwrap());

        // 随机选择URL
        let url_info = &codec.url_info[rand::rng().random_range(0..codec.url_info.len())];
        let url = format!("{}{}{}", url_info.host, codec.base_url, url_info.extra);

        let config = DownloadConfig {
            output_path: String::new(), // 将在start_download中设置
            overwrite: false,
            timeout: 30,
            retry_count: 3,
            codec: self.context.codec,
            format: self.context.format,
            quality: self.context.quality,
        };
        let http_downloader = HttpStreamDownloader::new(url.clone(), config, self.context.clone());

        Ok((url, DownloaderType::HttpStream(http_downloader)))
    }

    fn parse_hls_stream(&self, stream: &PlayStream) -> Result<(String, DownloaderType)> {
        if stream.format.is_empty() {
            anyhow::bail!("未找到合适的HLS流");
        }

        // 优先选择配置中的格式
        let format_stream = stream
            .format
            .iter()
            .find(|format| format.format_name == self.context.format)
            .or_else(|| stream.format.first())
            .ok_or_else(|| anyhow::anyhow!("未找到合适的视频格式"))?;

        if format_stream.codec.is_empty() {
            anyhow::bail!("未找到合适的视频编码");
        }

        // 优先按照设置选择编码格式
        let codec = format_stream
            .codec
            .iter()
            .find(|codec| codec.codec_name == self.context.codec)
            .unwrap_or_else(|| format_stream.codec.first().unwrap());

        // 随机选择URL
        let url_info = &codec.url_info[rand::rng().random_range(0..codec.url_info.len())];
        let url = format!("{}{}{}", url_info.host, codec.base_url, url_info.extra);

        // 创建HttpHlsDownloader
        let config = DownloadConfig {
            output_path: String::new(), // 将在start_download中设置
            overwrite: false,
            timeout: 30,
            retry_count: 3,
            codec: self.context.codec,
            format: self.context.format,
            quality: self.context.quality,
        };
        let hls_downloader = HttpHlsDownloader::new(url.clone(), config, self.context.clone());

        Ok((url, DownloaderType::HttpHls(hls_downloader)))
    }

    fn generate_filename(&self) -> Result<String> {
        let room_info = &self.context.room_info;
        let user_info = &self.context.user_info;

        let template = leon::Template::parse(DEFAULT_RECORD_NAME)
            .unwrap_or_else(|_| leon::Template::parse("{up_name}_{datetime}").unwrap());

        let live_time = NaiveDateTime::parse_from_str(&room_info.live_time, "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default();
        let live_time = live_time.and_local_timezone(Shanghai).unwrap();

        let values = DownloaderFilenameTemplate {
            up_name: user_info.uname.clone(),
            room_id: room_info.room_id,
            datetime: live_time.format("%Y-%m-%d %H点%M分").to_string(),
            room_title: room_info.title.clone(),
            room_description: room_info.description.clone(),
            room_area_name: room_info.area_name.clone(),
            date: live_time.format("%Y-%m-%d").to_string(),
        };

        let filename = template.render(&values).unwrap_or_default();
        Ok(filename)
    }

    fn resolve_file_path(&self, base_path: &str, filename: &str, ext: &str) -> Result<String> {
        const MAX_PARTS: u32 = 50; // 最大分片数量限制

        let initial_file_path = format!("{base_path}/{filename}.{ext}");
        let file_stem = std::path::Path::new(filename)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let folder_path = format!("{base_path}/{file_stem}");

        // 检查是否已经存在分P文件夹
        let folder_exists = std::path::Path::new(&folder_path).exists();
        let initial_file_exists = std::path::Path::new(&initial_file_path).exists();

        // 如果文件夹和原文件都不存在，返回原始路径
        if !folder_exists && !initial_file_exists {
            return Ok(initial_file_path);
        }

        // 如果存在分P文件夹或原文件存在，需要使用分P系统
        if folder_exists || initial_file_exists {
            // 创建文件夹（如果不存在）
            std::fs::create_dir_all(&folder_path).context("无法创建文件夹")?;

            // 扫描文件夹中现有的分P文件，找到所有现有的编号
            let mut existing_parts = Vec::new();

            if let Ok(folder) = std::fs::read_dir(&folder_path) {
                for entry in folder.flatten() {
                    let file_name_os = entry.file_name();
                    let file_name = file_name_os.to_string_lossy();

                    // 检查是否是我们的分P文件格式: {file_stem}_P{number}.{ext}
                    if let Some(name_without_ext) = file_name.strip_suffix(&format!(".{ext}")) {
                        if let Some(part_str) =
                            name_without_ext.strip_prefix(&format!("{file_stem}_P"))
                        {
                            // 尝试解析分P编号
                            if let Ok(part_num) = part_str.parse::<u32>() {
                                existing_parts.push(part_num);
                            }
                        }
                    }
                }
            }

            // 找到下一个可用的编号，但不超过最大限制
            let mut next_part_number = if existing_parts.is_empty() {
                1
            } else {
                existing_parts.sort();
                let max_existing = *existing_parts.iter().max().unwrap_or(&0);

                // 如果已达到最大分片数量，使用最后一个分片（P50）
                if max_existing >= MAX_PARTS {
                    MAX_PARTS
                } else {
                    max_existing + 1
                }
            };

            // 如果原文件存在且P1文件不存在，将原文件重命名为P1
            let first_part_name = format!("{file_stem}_P1.{ext}");
            let first_part_path = format!("{folder_path}/{first_part_name}");
            let mut new_file_name = format!("{file_stem}_P2.{ext}");
            #[allow(unused)]
            let mut new_file_path = format!("{folder_path}/{new_file_name}");

            if initial_file_exists && !std::path::Path::new(&first_part_path).exists() {
                std::fs::rename(&initial_file_path, &first_part_path).context(format!(
                    "重命名原文件失败: {initial_file_path} -> {first_part_path}"
                ))?;

                // 返回分P文件路径 P2
                next_part_number = 2;
                new_file_name = format!("{file_stem}_P{next_part_number}.{ext}");
                new_file_path = format!("{folder_path}/{new_file_name}");
            } else {
                // 返回分P文件路径
                new_file_name = format!("{file_stem}_P{next_part_number}.{ext}");
                new_file_path = format!("{folder_path}/{new_file_name}");
            }

            // 如果达到最大分片数量，记录日志提示
            if next_part_number == MAX_PARTS && existing_parts.contains(&MAX_PARTS) {
                eprintln!(
                    "⚠️  已达到最大分片数量({MAX_PARTS})，后续内容将附加到 P{MAX_PARTS} 文件中"
                );
            }

            Ok(new_file_path)
        } else {
            Ok(initial_file_path)
        }
    }
}
