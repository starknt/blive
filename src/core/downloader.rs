pub mod http_hls;
pub mod http_stream;

use crate::components::{RoomCard, RoomCardStatus};
use crate::core::downloader::{http_hls::HttpHlsDownloader, http_stream::HttpStreamDownloader};
use crate::core::http_client::HttpClient;
use crate::core::http_client::room::LiveRoomInfoData;
use crate::core::http_client::stream::{LiveRoomStreamUrl, PlayStream};
use crate::core::http_client::user::LiveUserInfo;
use crate::settings::{DEFAULT_RECORD_NAME, LiveProtocol, Quality, StreamCodec, VideoContainer};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
use gpui::{AsyncApp, WeakEntity};
use rand::Rng;
use std::{borrow::Cow, collections::VecDeque, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct DownloaderContext {
    pub entity: WeakEntity<RoomCard>,
    pub client: HttpClient,
    pub room_id: u64,
    pub quality: Quality,
    pub format: VideoContainer,
    pub codec: StreamCodec,
    // 内部状态
    stats: Arc<std::sync::Mutex<DownloadStats>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    // 事件队列 - 使用内部可变性
    event_queue: Arc<std::sync::Mutex<VecDeque<DownloadEvent>>>,
}

impl DownloaderContext {
    pub fn new(
        entity: WeakEntity<RoomCard>,
        client: HttpClient,
        room_id: u64,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
    ) -> Self {
        Self {
            entity,
            client,
            room_id,
            quality,
            format,
            codec,
            stats: Arc::new(std::sync::Mutex::new(DownloadStats::default())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_queue: Arc::new(std::sync::Mutex::new(VecDeque::new())),
        }
    }

    pub fn update_card_status(&self, cx: &mut AsyncApp, status: RoomCardStatus) {
        if let Some(entity) = self.entity.upgrade() {
            let _ = entity.update(cx, |card, cx| {
                card.status = status;
                cx.notify();
            });
        }
    }

    /// 推送事件到队列
    pub fn push_event(&self, event: DownloadEvent) {
        if let Ok(mut queue) = self.event_queue.lock() {
            queue.push_back(event);
        }
    }

    /// 处理队列中的所有事件，返回处理的事件数量
    pub fn process_events(&self, cx: &mut AsyncApp) -> usize {
        let mut processed = 0;

        if let Ok(mut queue) = self.event_queue.lock() {
            while let Some(event) = queue.pop_front() {
                self.handle_event(cx, event);
                processed += 1;
            }
        }

        processed
    }

    /// 处理单个事件
    fn handle_event(&self, cx: &mut AsyncApp, event: DownloadEvent) {
        // 记录日志
        self.log_event(&event);

        // 更新UI状态
        match &event {
            DownloadEvent::Started { .. } => {
                self.update_card_status(cx, RoomCardStatus::Recording(0.0));
            }
            DownloadEvent::Progress {
                download_speed_kbps,
                ..
            } => {
                self.update_card_status(cx, RoomCardStatus::Recording(*download_speed_kbps));
            }
            DownloadEvent::Error {
                error,
                is_recoverable,
            } => {
                let status = if *is_recoverable {
                    RoomCardStatus::Error(format!("网络异常，正在重连: {error}"))
                } else {
                    RoomCardStatus::Error(format!("录制失败: {error}"))
                };
                self.update_card_status(cx, status);
            }
            DownloadEvent::Reconnecting {
                attempt,
                delay_secs,
            } => {
                self.update_card_status(
                    cx,
                    RoomCardStatus::Error(format!(
                        "网络中断，第{attempt}次重连 ({delay_secs}秒后)"
                    )),
                );
            }
            DownloadEvent::Completed { .. } => {
                self.update_card_status(cx, RoomCardStatus::Waiting);
            }
            DownloadEvent::Paused => {
                self.update_card_status(cx, RoomCardStatus::Waiting);
            }
            DownloadEvent::Resumed => {
                self.update_card_status(cx, RoomCardStatus::Recording(0.0));
            }
        }
    }

    /// 记录事件日志
    fn log_event(&self, event: &DownloadEvent) {
        match event {
            DownloadEvent::Started { file_path } => {
                #[cfg(debug_assertions)]
                eprintln!("🎬 开始录制到: {file_path}");
            }
            DownloadEvent::Progress {
                bytes_downloaded,
                download_speed_kbps,
                duration_ms,
            } => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "📊 下载进度: {:.2}MB, {:.1}kb/s, {}秒",
                    *bytes_downloaded as f64 / 1024.0 / 1024.0,
                    download_speed_kbps,
                    duration_ms / 1000
                );
            }
            DownloadEvent::Error {
                error,
                is_recoverable,
            } => {
                if *is_recoverable {
                    eprintln!("⚠️  网络异常，正在重连: {error}");
                } else {
                    eprintln!("❌ 录制失败: {error}");
                }
            }
            DownloadEvent::Reconnecting {
                attempt,
                delay_secs,
            } => {
                eprintln!("🔄 网络中断，第{attempt}次重连 ({delay_secs}秒后)");
            }
            DownloadEvent::Completed {
                file_path,
                file_size,
            } => {
                let mb_size = *file_size as f64 / 1024.0 / 1024.0;
                eprintln!("✅ 录制完成: {file_path} ({mb_size:.2}MB)");
            }
            DownloadEvent::Paused => {
                eprintln!("⏸️  录制已暂停");
            }
            DownloadEvent::Resumed => {
                eprintln!("▶️  录制已恢复");
            }
        }
    }

    /// 启动事件处理任务
    pub fn start_event_processor(&self, cx: &mut AsyncApp) {
        let context = self.clone();

        cx.spawn(async move |cx| {
            while context.is_running() {
                // 每100ms处理一次事件队列
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;

                let processed = context.process_events(cx);

                // 如果没有事件处理且不在运行状态，退出循环
                if processed == 0 && !context.is_running() {
                    break;
                }
            }

            // 最后处理剩余的事件
            context.process_events(cx);
        })
        .detach();
    }

    /// 设置运行状态
    pub fn set_running(&self, running: bool) {
        self.is_running
            .store(running, std::sync::atomic::Ordering::Relaxed);
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 更新统计信息
    pub fn update_stats<F>(&self, updater: F)
    where
        F: FnOnce(&mut DownloadStats),
    {
        if let Ok(mut stats) = self.stats.lock() {
            updater(&mut stats);
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> DownloadStats {
        self.stats
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

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
    /// 下载暂停
    Paused,
    /// 下载恢复
    Resumed,
    /// 下载完成
    Completed { file_path: String, file_size: u64 },
    /// 下载错误
    Error { error: String, is_recoverable: bool },
    /// 网络重连中
    Reconnecting { attempt: u32, delay_secs: u64 },
}

// 下载统计信息
#[derive(Debug, Clone, Default)]
pub struct DownloadStats {
    pub bytes_downloaded: u64,
    pub download_speed_kbps: f32,
    pub duration_ms: u64,
    pub reconnect_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloaderError {
    #[error("网络错误: {0}")]
    NetworkError(String),

    #[error("文件系统错误: {0}")]
    FileSystemError(String),
}

pub trait Downloader {
    /// 开始下载
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()>;

    /// 停止下载
    fn stop(&mut self) -> Result<()>;

    /// 暂停下载
    fn pause(&mut self) -> Result<()>;

    /// 恢复下载
    fn resume(&mut self) -> Result<()>;

    /// 获取下载状态
    fn status(&self) -> DownloadStatus;

    /// 获取下载统计信息
    fn stats(&self) -> DownloadStats;
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    /// 未开始
    NotStarted,
    /// 下载中
    Downloading,
    /// 已暂停
    Paused,
    /// 已完成
    Completed,
    /// 重连中
    Reconnecting,
    /// 错误
    Error(String),
}

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// 输出路径
    pub output_path: String,
    /// 是否覆盖
    pub overwrite: bool,
    /// 超时时间（秒）
    pub timeout: u64,
    /// 重试次数
    pub retry_count: u32,
    /// 编码
    pub codec: StreamCodec,
    /// 视频容器
    pub format: VideoContainer,
    /// 画质
    pub quality: Quality,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            output_path: "download".to_string(),
            overwrite: false,
            timeout: 30,
            retry_count: 3,
            codec: StreamCodec::default(),
            format: VideoContainer::default(),
            quality: Quality::default(),
        }
    }
}

pub enum DownloaderType {
    HttpStream(HttpStreamDownloader),
    HttpHls(HttpHlsDownloader),
}

pub struct DownloaderFilenameTemplate {
    pub up_name: String,
    pub room_id: u64,
    pub room_title: String,
    pub room_description: String,
    pub room_area_name: String,
    pub date: String,
    pub datetime: String,
}

impl leon::Values for DownloaderFilenameTemplate {
    fn get_value(&self, key: &str) -> Option<Cow<'_, str>> {
        match key {
            "up_name" => Some(Cow::Borrowed(&self.up_name)),
            "room_id" => Some(Cow::Owned(self.room_id.to_string())),
            "datetime" => Some(Cow::Borrowed(&self.datetime)),
            "room_title" => Some(Cow::Borrowed(&self.room_title)),
            "room_description" => Some(Cow::Borrowed(&self.room_description)),
            "room_area_name" => Some(Cow::Borrowed(&self.room_area_name)),
            "date" => Some(Cow::Borrowed(&self.date)),
            _ => None,
        }
    }
}

pub struct BLiveDownloader {
    context: DownloaderContext,
    downloader: Option<DownloaderType>,
    // 网络重连相关字段
    max_reconnect_attempts: u32,
    reconnect_delay: Duration,
    is_auto_reconnect: bool,
}

impl BLiveDownloader {
    pub fn new(
        room_id: u64,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
        client: HttpClient,
        entity: WeakEntity<RoomCard>,
    ) -> Self {
        let context: DownloaderContext =
            DownloaderContext::new(entity, client, room_id, quality, format, codec);

        Self {
            context,
            downloader: None,
            max_reconnect_attempts: u32::MAX,        // 无限重试
            reconnect_delay: Duration::from_secs(1), // 初始延迟1秒
            is_auto_reconnect: true,                 // 是否启用自动重连
        }
    }

    fn update_card_status(&self, cx: &mut AsyncApp, status: RoomCardStatus) {
        self.context.update_card_status(cx, status);
    }

    /// 设置重连参数
    pub fn set_reconnect_config(
        &mut self,
        max_attempts: u32,
        initial_delay: Duration,
        auto_reconnect: bool,
    ) {
        self.max_reconnect_attempts = max_attempts;
        self.reconnect_delay = initial_delay;
        self.is_auto_reconnect = auto_reconnect;
    }

    /// 计算指数退避延迟，最大等待时间30分钟
    fn calculate_backoff_delay(&self, retry_count: u32) -> Duration {
        const MAX_DELAY: Duration = Duration::from_secs(30 * 60); // 30分钟

        // 指数退避：1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 1800(30分钟)
        let exponential_delay = self.reconnect_delay * (2_u32.pow(retry_count.min(10)));

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
                .get_live_room_stream_url(self.context.room_id, self.context.quality.to_quality())
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
        let http_downloader = HttpStreamDownloader::new(
            url.clone(),
            config,
            self.context.client.clone(),
            self.context.clone(),
        );

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

    fn generate_filename(
        &self,
        room_info: &LiveRoomInfoData,
        user_info: &LiveUserInfo,
    ) -> Result<String> {
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
        let mut final_path = format!("{base_path}/{filename}.{ext}");
        let mut part_number = 1;

        while std::path::Path::new(&final_path).exists() {
            // 创建文件夹（去掉扩展名）
            let file_stem = std::path::Path::new(filename)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let folder_path = format!("{base_path}/{file_stem}");

            // 创建文件夹
            std::fs::create_dir_all(&folder_path).context("无法创建文件夹")?;

            // 检查文件夹中已有的文件，找到下一个可用的编号
            let folder = std::fs::read_dir(&folder_path).unwrap_or_else(|_| {
                std::fs::create_dir_all(&folder_path).unwrap_or_default();
                std::fs::read_dir(&folder_path).unwrap_or_else(|_| {
                    panic!("无法创建或读取文件夹: {folder_path}");
                })
            });

            let mut existing_parts = Vec::new();
            for entry in folder.flatten() {
                if let Some(file_name) = entry
                    .file_name()
                    .to_string_lossy()
                    .strip_suffix(&format!(".{ext}"))
                    && let Some(part_str) = file_name.strip_suffix(&format!("_P{part_number}"))
                    && part_str == file_stem
                {
                    existing_parts.push(part_number);
                }
            }

            // 找到下一个可用的编号
            while existing_parts.contains(&part_number) {
                part_number += 1;
            }

            // 重命名旧文件
            let old_file_path = final_path.clone();
            let new_file_name = format!("{file_stem}_P{part_number}.{ext}");
            let new_file_path = format!("{folder_path}/{new_file_name}");

            std::fs::rename(&old_file_path, &new_file_path).context(format!(
                "重命名文件失败: {old_file_path} -> {new_file_path}"
            ))?;

            // 更新文件路径为新的编号
            final_path = format!("{}/{}_P{}.{}", folder_path, file_stem, part_number + 1, ext);
            part_number += 1;
        }

        Ok(final_path)
    }

    pub async fn start_download(
        &mut self,
        cx: &mut AsyncApp,
        room_info: &LiveRoomInfoData,
        user_info: &LiveUserInfo,
        record_dir: &str,
    ) -> Result<()> {
        // 设置运行状态
        self.context.set_running(true);

        // 启动事件处理器
        self.context.start_event_processor(cx);

        // 获取流信息
        let stream_info = self.get_stream_info().await?;

        // 解析下载URL和选择下载器类型
        let (url, downloader_type) = self.parse_stream_url(&stream_info)?;

        // 生成文件名
        let filename = self.generate_filename(room_info, user_info)?;

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
                let downloader = HttpStreamDownloader::new(
                    url,
                    config,
                    self.context.client.clone(),
                    self.context.clone(),
                );

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
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            },
            DownloaderType::HttpHls(downloader) => match downloader.start(cx) {
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            },
        }

        self.downloader = Some(final_downloader);

        Ok(())
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

    /// 带重连的下载方法
    pub async fn start_download_with_retry(
        &mut self,
        cx: &mut AsyncApp,
        room_info: &LiveRoomInfoData,
        user_info: &LiveUserInfo,
        record_dir: &str,
    ) -> Result<()> {
        let mut retry_count = 0;

        loop {
            match self
                .start_download(cx, room_info, user_info, record_dir)
                .await
            {
                Ok(_) => {
                    // 下载成功启动，重置重连计数
                    self.context.update_stats(|stats| {
                        stats.reconnect_count = 0;
                    });

                    // 更新UI状态为录制中
                    self.update_card_status(cx, RoomCardStatus::Recording(0.0));

                    // 下载成功启动，现在监控下载状态
                    if self.is_auto_reconnect {
                        // self.monitor_download_with_reconnect(cx, room_info, user_info, record_dir)
                        //     .await?;
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
                            "网络中断，第{}次重连 ({}秒后)",
                            retry_count,
                            delay.as_secs()
                        )),
                    );

                    // 发送重连事件
                    self.context.push_event(DownloadEvent::Reconnecting {
                        attempt: retry_count,
                        delay_secs: delay.as_secs(),
                    });

                    // 等待一段时间后重试 - 使用异步定时器
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
                        error: format!("非网络错误: {e}"),
                        is_recoverable: false,
                    });

                    return Err(e);
                }
            }
        }
    }

    pub fn stop(&mut self) {
        // 设置停止状态
        self.context.set_running(false);

        // 发送暂停事件
        self.context.push_event(DownloadEvent::Paused);

        if let Some(ref mut downloader) = self.downloader {
            match downloader {
                DownloaderType::HttpStream(downloader) => {
                    let _ = downloader.stop();
                }
                DownloaderType::HttpHls(downloader) => {
                    let _ = downloader.stop();
                }
            }
        }
    }
}
