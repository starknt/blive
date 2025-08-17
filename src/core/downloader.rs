pub mod context;
pub mod error;
pub mod http_hls;
pub mod http_stream;
pub mod stats;
pub mod template;
pub mod utils;

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
use gpui::AsyncApp;
use rand::Rng;
pub use stats::DownloadStats;
use std::sync::Mutex;

pub const REFERER: &str = "https://live.bilibili.com/";
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub trait Downloader {
    /// 开始下载
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()>;

    /// 停止下载
    fn stop(&mut self) -> impl std::future::Future<Output = Result<()>> + Send;

    fn is_running(&self) -> bool;

    fn set_running(&self, running: bool);
}

#[derive(Debug)]
pub enum DownloaderType {
    HttpStream(Option<HttpStreamDownloader>),
    HttpHls(Option<HttpHlsDownloader>),
}

#[derive(Debug)]
pub struct BLiveDownloader {
    pub context: DownloaderContext,
    downloader: Mutex<Option<DownloaderType>>,
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
                log_user_action("录制目录创建成功", Some(&format!("路径: {record_dir}")));
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

    pub async fn start(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        match self.start_download(cx, record_dir).await {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
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

    pub async fn restart(&self, cx: &mut AsyncApp, record_dir: &str) -> Result<()> {
        self.stop().await;
        self.start(cx, record_dir).await
    }

    pub fn is_running(&self) -> bool {
        self.context.is_running()
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
        room_id: u64,
    ) -> Self {
        let context: DownloaderContext = DownloaderContext::new(
            room_id, client, room_info, user_info, strategy, quality, format, codec,
        );

        Self {
            context,
            downloader: Mutex::new(None),
        }
    }

    /// 获取下载统计信息
    pub fn get_download_stats(&self) -> Option<DownloadStats> {
        Some(self.context.get_stats())
    }

    /// 获取直播流信息
    async fn get_stream_info(&self) -> Result<LiveRoomStreamUrl> {
        match self
            .context
            .client
            .get_live_room_stream_url(
                self.context.room_info.room_id,
                self.context.quality.to_quality(),
            )
            .await
        {
            Ok(stream_info) => Ok(stream_info),
            Err(e) => Err(e),
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
