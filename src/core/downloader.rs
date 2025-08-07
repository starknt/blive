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
use crate::log_user_action;
use crate::settings::{
    DEFAULT_RECORD_NAME, LiveProtocol, Quality, Strategy, StreamCodec, VideoContainer,
};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
pub use context::{DownloadConfig, DownloaderContext};
use gpui::{AsyncApp, WeakEntity};
use rand::Rng;
pub use stats::DownloadStats;
use std::sync::Mutex;
use std::time::Duration;
use try_lock::TryLock;

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
    Error(DownloaderError),
}

pub enum DownloaderType {
    HttpStream(Option<HttpStreamDownloader>),
    HttpHls(Option<HttpHlsDownloader>),
}

pub struct BLiveDownloader {
    pub context: DownloaderContext,
    downloader: Mutex<Option<DownloaderType>>,
    max_reconnect_attempts: TryLock<u32>,
    reconnect_delay: TryLock<Duration>,
    is_auto_reconnect: TryLock<bool>,
    reconnect_manager: TryLock<ReconnectManager>,
}

#[derive(Debug)]
struct ReconnectManager {
    current_attempt: u32,
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    last_error: Option<String>,
    consecutive_successes: u32,
    last_reconnect_time: Option<std::time::Instant>,
}

impl ReconnectManager {
    fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            current_attempt: 0,
            max_attempts,
            base_delay,
            max_delay,
            last_error: None,
            consecutive_successes: 0,
            last_reconnect_time: None,
        }
    }

    fn should_reconnect(&self) -> bool {
        self.current_attempt < self.max_attempts
    }

    fn increment_attempt(&mut self) {
        self.current_attempt += 1;
        self.last_reconnect_time = Some(std::time::Instant::now());
    }

    fn reset_attempts(&mut self) {
        self.current_attempt = 0;
        self.consecutive_successes += 1;
        self.last_error = None;
    }

    fn set_error(&mut self, error: String) {
        self.last_error = Some(error);
    }

    fn calculate_delay(&self) -> Duration {
        // 指数退避算法，带随机抖动
        let exponential_delay = self.base_delay * (2_u32.pow(self.current_attempt.min(10)));
        let jitter = rand::rng().random_range(0.8..1.2);

        let delay = Duration::from_secs_f64(exponential_delay.as_secs_f64() * jitter);

        delay.min(self.max_delay)
    }
}

impl BLiveDownloader {
    async fn start_download(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        self.context.init();

        // 获取流信息
        let stream_info = self.get_stream_info().await?;

        // 解析下载URL和选择下载器类型
        let (url, downloader_type, format, codec) = self.parse_stream_url(&stream_info)?;

        // 生成文件名
        let filename = self.generate_filename()?;

        // 获取文件扩展名
        let ext = format.ext();

        // 确保录制目录存在
        if !std::path::Path::new(record_dir).exists() {
            if std::fs::create_dir_all(record_dir).is_ok() {
                log_user_action("录制目录创建成功", Some(&format!("路径: {}", record_dir)));
            } else {
                return Err(anyhow::anyhow!("无法创建录制目录: {}", record_dir));
            }
        }

        // 处理文件路径冲突
        let file_path = self.resolve_file_path(record_dir, &filename, ext)?;

        let config = DownloadConfig {
            output_path: file_path.clone(),
            overwrite: false,
            timeout: 30,
            retry_count: 3,
            codec,
            format,
            quality: self.context.quality,
            strategy: self.context.strategy,
        };

        // 根据下载器类型创建具体的下载器
        let mut final_downloader = match downloader_type {
            DownloaderType::HttpStream(_) => {
                let downloader = HttpStreamDownloader::new(url, config, self.context.clone());

                DownloaderType::HttpStream(Some(downloader))
            }
            DownloaderType::HttpHls(_) => {
                let downloader = HttpHlsDownloader::new(url, config, self.context.clone());

                DownloaderType::HttpHls(Some(downloader))
            }
        };

        match &mut final_downloader {
            DownloaderType::HttpStream(Some(downloader)) => match downloader.start(cx) {
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
            DownloaderType::HttpHls(Some(downloader)) => match downloader.start(cx) {
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
            DownloaderType::HttpHls(None) | DownloaderType::HttpStream(None) => {
                return Err(anyhow::anyhow!("未能创建下载器"));
            }
        }

        self.downloader
            .try_lock()
            .unwrap()
            .replace(final_downloader);

        Ok(())
    }

    /// 统一的重连方法
    async fn attempt_reconnect(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        let mut manager = self.reconnect_manager.try_lock().unwrap();

        if !manager.should_reconnect() {
            return Err(anyhow::anyhow!("已达到最大重连次数"));
        }

        manager.increment_attempt();
        let attempt = manager.current_attempt;
        let delay = manager.calculate_delay();

        self.update_card_status(
            cx,
            RoomCardStatus::Error(format!(
                "网络中断，第{}次重连 ({}秒后)",
                attempt,
                delay.as_secs()
            )),
        );

        // 发送重连事件
        self.context.push_event(DownloadEvent::Reconnecting {
            attempt,
            delay_secs: delay.as_secs(),
        });

        // 更新统计信息
        self.context.update_stats(|stats| {
            stats.reconnect_count = attempt;
        });

        drop(manager); // 释放锁

        eprintln!("🔄 网络异常，正在尝试重连 (第{attempt}次，等待{delay:?})");

        // 等待延迟时间
        cx.background_executor().timer(delay).await;

        // 尝试重新启动下载
        match self.start_download(cx, record_dir).await {
            Ok(_) => {
                // 重连成功，重置计数器
                let mut manager = self.reconnect_manager.try_lock().unwrap();
                manager.reset_attempts();

                eprintln!("✅ 重连成功！");
                self.update_card_status(cx, RoomCardStatus::Recording(0.0));
                Ok(())
            }
            Err(e) => {
                // 重连失败，记录错误
                let mut manager = self.reconnect_manager.try_lock().unwrap();
                manager.set_error(e.to_string());

                eprintln!("❌ 重连失败: {e}");
                Err(e)
            }
        }
    }

    /// 改进的启动方法
    pub async fn start(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        // 重置重连管理器
        {
            let mut manager = self.reconnect_manager.try_lock().unwrap();
            manager.current_attempt = 0;
            manager.consecutive_successes = 0;
        }

        // 尝试启动下载
        match self.start_download(cx, record_dir).await {
            Ok(_) => {
                // 下载成功启动
                self.context.update_stats(|stats| {
                    stats.reconnect_count = 0;
                });

                self.update_card_status(cx, RoomCardStatus::Recording(0.0));

                // 如果启用自动重连，启动监控
                if self.is_auto_reconnect() {
                    self.monitor_download_status(cx, record_dir).await?;
                }

                Ok(())
            }
            Err(e) => {
                // 检查是否为网络错误
                if Self::is_network_error(&e) {
                    // 网络错误，尝试重连
                    self.attempt_reconnect(cx, record_dir).await
                } else {
                    // 非网络错误，直接返回
                    eprintln!("非网络错误，停止重连: {e}");
                    self.update_card_status(cx, RoomCardStatus::Error(format!("录制失败: {e}")));

                    self.context.push_event(DownloadEvent::Error {
                        error: DownloaderError::InvalidRecordingConfig {
                            field: "stream_url".to_string(),
                            value: "unavailable".to_string(),
                            reason: format!("非网络错误: {e}"),
                        },
                    });

                    Err(e)
                }
            }
        }
    }

    pub async fn stop(&self) {
        // 设置停止状态
        self.context.set_running(false);
        self.context.set_status(DownloadStatus::NotStarted);

        {
            let mut downloader_guard = self.downloader.lock().unwrap();
            if let Some(ref mut downloader) = downloader_guard.as_mut() {
                match downloader {
                    DownloaderType::HttpStream(downloader) => {
                        if let Some(downloader) = downloader {
                            let _ = downloader.stop().await;
                        }
                    }
                    DownloaderType::HttpHls(downloader) => {
                        if let Some(downloader) = downloader {
                            let _ = downloader.stop().await;
                        }
                    }
                }
            }
        }
    }

    /// 改进的状态监控方法
    pub async fn monitor_download_status(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        let mut last_status = self.context.get_status();
        let mut status_check_interval = Duration::from_secs(1); // 初始检查间隔1秒

        while self.context.is_running() {
            let current_status = self.context.get_status();

            // 状态发生变化时立即处理
            if current_status != last_status {
                match current_status {
                    DownloadStatus::Error(ref error) => {
                        let is_network_error =
                            Self::is_network_error(&anyhow::anyhow!("{}", error));

                        if is_network_error && self.is_auto_reconnect() {
                            // 停止当前下载器
                            self.stop().await;

                            // 尝试重连
                            if let Err(e) = self.attempt_reconnect(cx, record_dir).await {
                                eprintln!("重连失败，停止监控: {e}");
                                break;
                            }
                        } else {
                            // 非网络错误，停止监控
                            eprintln!("非网络错误，停止监控: {error}");
                            break;
                        }
                    }
                    DownloadStatus::Completed => {
                        eprintln!("下载完成，停止监控");
                        break;
                    }
                    DownloadStatus::Downloading => {
                        // 下载正常，更新进度
                        if let Some(stats) = self.get_download_stats() {
                            self.context.push_event(DownloadEvent::Progress {
                                bytes_downloaded: stats.bytes_downloaded,
                                download_speed_kbps: stats.download_speed_kbps,
                                duration_ms: stats.duration_ms,
                            });
                        }

                        // 下载正常时，可以增加检查间隔
                        status_check_interval = Duration::from_secs(2);
                    }
                    DownloadStatus::Reconnecting => {
                        // 已经在重连中，等待重连完成
                        status_check_interval = Duration::from_secs(1);
                    }
                    DownloadStatus::NotStarted => {
                        // 下载器未启动，尝试重新启动
                        eprintln!("⚠️  下载器未启动，尝试重新启动");

                        if let Err(e) = self.start_download(cx, record_dir).await {
                            eprintln!("❌ 重新启动失败: {e}");
                            if Self::is_network_error(&e) {
                                // 网络错误，尝试重连
                                if let Err(e) = self.attempt_reconnect(cx, record_dir).await {
                                    eprintln!("重连失败，停止监控: {e}");
                                    break;
                                }
                            } else {
                                // 非网络错误，停止监控
                                break;
                            }
                        } else {
                            eprintln!("✅ 重新启动成功");
                        }
                    }
                }

                last_status = current_status;
            }

            // 等待后再次检查
            cx.background_executor().timer(status_check_interval).await;
        }

        Ok(())
    }
}

impl BLiveDownloader {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        room_info: LiveRoomInfoData,
        user_info: LiveUserInfo,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
        strategy: Strategy,
        client: HttpClient,
        entity: WeakEntity<RoomCard>,
    ) -> Self {
        let context: DownloaderContext = DownloaderContext::new(
            entity, client, room_info, user_info, strategy, quality, format, codec,
        );

        let reconnect_manager = ReconnectManager::new(
            u32::MAX,                     // 无限重试
            Duration::from_secs(1),       // 初始延迟1秒
            Duration::from_secs(30 * 60), // 最大延迟30分钟
        );

        Self {
            context,
            downloader: Mutex::new(None),
            max_reconnect_attempts: TryLock::new(u32::MAX),
            reconnect_delay: TryLock::new(Duration::from_secs(1)),
            is_auto_reconnect: TryLock::new(true),
            reconnect_manager: TryLock::new(reconnect_manager),
        }
    }

    fn update_card_status(&self, cx: &mut AsyncApp, status: RoomCardStatus) {
        self.context.update_card_status(cx, status);
    }

    fn is_auto_reconnect(&self) -> bool {
        *self.is_auto_reconnect.try_lock().unwrap()
    }

    /// 改进的网络错误检测
    fn is_network_error(error: &anyhow::Error) -> bool {
        let error_str = error.to_string().to_lowercase();

        // 更精确的网络错误检测
        let network_keywords = [
            "network",
            "connection",
            "timeout",
            "dns",
            "socket",
            "unreachable",
            "reset",
            "refused",
            "无法连接",
            "连接被重置",
            "连接超时",
            "网络",
            "请求失败",
            "无法读取响应体",
            "connection refused",
            "connection reset",
            "no route to host",
            "host unreachable",
            "network unreachable",
            "connection timed out",
            "read timeout",
            "write timeout",
            "tcp connection",
            "udp connection",
            "http",
            "https",
            "ssl",
            "tls",
        ];

        network_keywords
            .iter()
            .any(|keyword| error_str.contains(keyword))
    }

    /// 获取下载统计信息
    fn get_download_stats(&self) -> Option<DownloadStats> {
        Some(self.context.get_stats())
    }

    /// 设置重连配置
    pub fn set_reconnect_config(
        &mut self,
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        auto_reconnect: bool,
    ) {
        let mut max_reconnect_attempts = self.max_reconnect_attempts.try_lock().unwrap();
        let mut reconnect_delay = self.reconnect_delay.try_lock().unwrap();
        let mut is_auto_reconnect = self.is_auto_reconnect.try_lock().unwrap();
        let mut reconnect_manager = self.reconnect_manager.try_lock().unwrap();

        *max_reconnect_attempts = max_attempts;
        *reconnect_delay = initial_delay;
        *is_auto_reconnect = auto_reconnect;

        // 更新重连管理器配置
        reconnect_manager.max_attempts = max_attempts;
        reconnect_manager.base_delay = initial_delay;
        reconnect_manager.max_delay = max_delay;
    }

    /// 获取重连统计信息
    pub fn get_reconnect_stats(&self) -> (u32, u32, Option<String>) {
        let manager = self.reconnect_manager.try_lock().unwrap();
        (
            manager.current_attempt,
            manager.consecutive_successes,
            manager.last_error.clone(),
        )
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
                    let delay = {
                        let manager = self.reconnect_manager.try_lock().unwrap();
                        manager.calculate_delay()
                    };

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
    ) -> Result<(String, DownloaderType, VideoContainer, StreamCodec)> {
        let playurl_info = stream_info
            .playurl_info
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("未找到播放信息"))?;

        match self.context.strategy {
            Strategy::LowCost => {
                // 优先尝试http_stream协议
                if let Some(stream) = playurl_info
                    .playurl
                    .stream
                    .iter()
                    .find(|stream| stream.protocol_name == LiveProtocol::HttpStream)
                {
                    return self.parse_http_stream(stream);
                }

                // 如果没有http_stream，尝试http_hls协议
                if let Some(stream) = playurl_info
                    .playurl
                    .stream
                    .iter()
                    .find(|stream| stream.protocol_name == LiveProtocol::default())
                {
                    return self.parse_http_stream(stream);
                }
            }
            Strategy::PriorityConfig => {
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
            }
        }

        anyhow::bail!("未找到合适的直播流协议");
    }

    fn parse_http_stream(
        &self,
        stream: &PlayStream,
    ) -> Result<(String, DownloaderType, VideoContainer, StreamCodec)> {
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

        Ok((
            url,
            DownloaderType::HttpStream(None),
            format_stream.format_name,
            codec.codec_name,
        ))
    }

    fn parse_hls_stream(
        &self,
        stream: &PlayStream,
    ) -> Result<(String, DownloaderType, VideoContainer, StreamCodec)> {
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

        Ok((
            url,
            DownloaderType::HttpHls(None),
            format_stream.format_name,
            codec.codec_name,
        ))
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
