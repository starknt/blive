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
use std::{borrow::Cow, time::Duration};

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

    /// 获取下载状态
    fn status(&self) -> DownloadStatus;
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
    /// 是否启用GPU加速
    pub hwaccel: bool,
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
            hwaccel: true,
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
    pub(crate) room_id: u64,
    pub(crate) quality: Quality,
    pub(crate) format: VideoContainer,
    pub(crate) codec: StreamCodec,
    pub(crate) client: HttpClient,
    pub(crate) downloader: Option<DownloaderType>,
    pub(crate) entity: WeakEntity<RoomCard>,
    // 网络重连相关字段
    pub(crate) max_reconnect_attempts: u32,
    pub(crate) reconnect_delay: Duration,
    pub(crate) is_auto_reconnect: bool,
    pub(crate) hwaccel: bool,
}

impl BLiveDownloader {
    pub fn new(
        room_id: u64,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
        hwaccel: bool,
        client: HttpClient,
        entity: WeakEntity<RoomCard>,
    ) -> Self {
        Self {
            room_id,
            quality,
            format,
            codec,
            client,
            downloader: None,
            entity,
            max_reconnect_attempts: u32::MAX,        // 无限重试
            reconnect_delay: Duration::from_secs(1), // 初始延迟1秒
            is_auto_reconnect: true,                 // 是否启用自动重连
            hwaccel,
        }
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
                .client
                .get_live_room_stream_url(self.room_id, self.quality.to_quality())
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
            .find(|format| format.format_name == self.format)
            .or_else(|| stream.format.first())
            .ok_or_else(|| anyhow::anyhow!("未找到合适的视频格式"))?;

        if format_stream.codec.is_empty() {
            anyhow::bail!("未找到合适的视频编码");
        }

        // 优先按照设置选择编码格式
        let codec = format_stream
            .codec
            .iter()
            .find(|codec| codec.codec_name == self.codec)
            .unwrap_or_else(|| format_stream.codec.first().unwrap());

        // 随机选择URL
        let url_info = &codec.url_info[rand::rng().random_range(0..codec.url_info.len())];
        let url = format!("{}{}{}", url_info.host, codec.base_url, url_info.extra);

        let config = DownloadConfig {
            output_path: String::new(), // 将在start_download中设置
            overwrite: false,
            timeout: 30,
            retry_count: 3,
            codec: self.codec,
            format: self.format,
            quality: self.quality,
            hwaccel: self.hwaccel,
        };
        let http_downloader = HttpStreamDownloader::new(
            url.clone(),
            config,
            self.client.clone(),
            self.entity.clone(),
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
            .find(|format| format.format_name == self.format)
            .or_else(|| stream.format.first())
            .ok_or_else(|| anyhow::anyhow!("未找到合适的视频格式"))?;

        if format_stream.codec.is_empty() {
            anyhow::bail!("未找到合适的视频编码");
        }

        // 优先按照设置选择编码格式
        let codec = format_stream
            .codec
            .iter()
            .find(|codec| codec.codec_name == self.codec)
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
            codec: self.codec,
            format: self.format,
            quality: self.quality,
            hwaccel: self.hwaccel,
        };
        let hls_downloader = HttpHlsDownloader::new(url.clone(), config, self.entity.clone());

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
        // 获取流信息
        let stream_info = self.get_stream_info().await?;

        // 解析下载URL和选择下载器类型
        let (url, downloader_type) = self.parse_stream_url(&stream_info)?;

        // 生成文件名
        let filename = self.generate_filename(room_info, user_info)?;

        // 获取文件扩展名
        let ext = self.format.ext();

        // 处理文件路径冲突
        let file_path = self.resolve_file_path(record_dir, &filename, ext)?;

        // 根据下载器类型创建具体的下载器
        let mut final_downloader = match downloader_type {
            DownloaderType::HttpStream(_) => {
                let config = DownloadConfig {
                    output_path: file_path,
                    overwrite: false,
                    timeout: 30,
                    retry_count: 3,
                    codec: self.codec,
                    format: self.format,
                    quality: self.quality,
                    hwaccel: self.hwaccel,
                };
                DownloaderType::HttpStream(HttpStreamDownloader::new(
                    url,
                    config,
                    self.client.clone(),
                    self.entity.clone(),
                ))
            }
            DownloaderType::HttpHls(_) => {
                let config = DownloadConfig {
                    output_path: file_path,
                    overwrite: false,
                    timeout: 30,
                    retry_count: 3,
                    codec: self.codec,
                    format: self.format,
                    quality: self.quality,
                    hwaccel: self.hwaccel,
                };
                DownloaderType::HttpHls(HttpHlsDownloader::new(url, config, self.entity.clone()))
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

        // 保存下载器引用
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
                    // 下载成功启动，现在监控下载状态
                    if self.is_auto_reconnect {
                        self.monitor_download_with_reconnect(cx, room_info, user_info, record_dir)
                            .await?;
                    }
                    return Ok(());
                }
                Err(e) if Self::is_network_error(&e) => {
                    retry_count += 1;
                    let delay = self.calculate_backoff_delay(retry_count);

                    eprintln!("网络异常，正在尝试重连 (第{retry_count}次，等待{delay:?}): {e}");

                    // 更新UI状态
                    let _ = self.entity.update(cx, |card, cx| {
                        card.error_message = Some(format!(
                            "网络异常，正在重连... (第{retry_count}次，等待{delay:?})"
                        ));
                        cx.notify();
                    });

                    // 等待一段时间后重试 - 使用异步定时器
                    cx.background_executor().timer(delay).await;
                    continue;
                }
                Err(e) => {
                    // 非网络错误，直接返回
                    eprintln!("非网络错误，停止重连: {e}");
                    let _ = self.entity.update(cx, |card, cx| {
                        card.error_message = Some(format!("非网络错误: {e}"));
                        cx.notify();
                    });
                    return Err(e);
                }
            }
        }
    }

    /// 监控下载状态并在需要时重连
    async fn monitor_download_with_reconnect(
        &mut self,
        cx: &mut AsyncApp,
        room_info: &LiveRoomInfoData,
        user_info: &LiveUserInfo,
        record_dir: &str,
    ) -> Result<()> {
        if !self.is_auto_reconnect {
            return Ok(());
        }

        let entity = self.entity.clone();
        let room_id = self.room_id;
        let quality = self.quality;
        let format = self.format;
        let codec = self.codec;
        let hwaccel = self.hwaccel;
        let client = self.client.clone();
        let initial_delay = self.reconnect_delay;
        let room_info = room_info.clone();
        let user_info = user_info.clone();
        let record_dir = record_dir.to_string();

        cx.spawn(async move |cx| {
            let mut reconnect_count = 0;

            loop {
                // 等待一段时间后检查状态
                cx.background_executor()
                    .timer(Duration::from_secs(30))
                    .await;

                // 检查下载器状态
                let should_reconnect = entity
                    .update(cx, |card, _| matches!(&card.status, RoomCardStatus::Error))
                    .unwrap_or(false);

                if should_reconnect {
                    reconnect_count += 1;

                    // 计算指数退避延迟
                    let delay = {
                        const MAX_DELAY: Duration = Duration::from_secs(30 * 60); // 30分钟
                        let exponential_delay =
                            initial_delay * (2_u32.pow(reconnect_count.min(10)));
                        if exponential_delay > MAX_DELAY {
                            MAX_DELAY
                        } else {
                            exponential_delay
                        }
                    };

                    eprintln!(
                        "检测到下载异常，尝试重新连接 (第{reconnect_count}次，等待{delay:?})"
                    );

                    // 更新UI状态
                    let _ = entity.update(cx, |card, cx| {
                        card.error_message = Some(format!(
                            "检测到异常，正在重连... (第{reconnect_count}次，等待{delay:?})"
                        ));
                        cx.notify();
                    });

                    // 创建新的下载器实例
                    let mut new_downloader = BLiveDownloader::new(
                        room_id,
                        quality,
                        format,
                        codec,
                        hwaccel,
                        client.clone(),
                        entity.clone(),
                    );
                    new_downloader.set_reconnect_config(u32::MAX, initial_delay, false); // 避免嵌套监控

                    // 等待指数退避延迟
                    cx.background_executor().timer(delay).await;

                    // 尝试重新开始下载
                    match new_downloader
                        .start_download(cx, &room_info, &user_info, &record_dir)
                        .await
                    {
                        Ok(_) => {
                            eprintln!("重连成功！");
                            let _ = entity.update(cx, |card, cx| {
                                card.error_message = Some("重连成功，继续录制...".to_string());
                                cx.notify();
                            });
                            reconnect_count = 0; // 重置重连计数
                        }
                        Err(e) => {
                            eprintln!("重连失败: {e}");
                            // 继续循环，无限重试
                        }
                    }
                }
            }

            // 这里不会到达，因为是无限循环
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        })
        .detach();

        Ok(())
    }

    pub fn stop(&mut self) {
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
