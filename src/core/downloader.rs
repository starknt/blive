pub mod http_hls;
pub mod http_stream;

use std::{borrow::Cow, time::Duration};

use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
use gpui::AsyncApp;
use rand::Rng;

use crate::{
    HttpHlsDownloader, HttpStreamDownloader,
    api::{HttpClient, room::LiveRoomInfoData, stream::LiveRoomStreamUrl, user::LiveUserInfo},
    settings::{DEFAULT_RECORD_NAME, StreamCodec, VideoFormat},
};

pub trait Downloader {
    /// 开始下载
    fn start(&mut self, cx: &mut AsyncApp) -> Result<()>;

    /// 停止下载
    fn stop(&mut self) -> Result<()>;

    /// 获取下载状态
    fn status(&self) -> DownloadStatus;
}

#[derive(Debug, Clone)]
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
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            output_path: "download".to_string(),
            overwrite: false,
            timeout: 30,
            retry_count: 3,
        }
    }
}

#[derive(Debug)]
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

pub struct BilibiliDownloader {
    pub(crate) room_id: u64,
    pub(crate) quality: u32,
    pub(crate) format: VideoFormat,
    pub(crate) codec: StreamCodec,
    pub(crate) client: HttpClient,
    pub(crate) downloader: Option<DownloaderType>,
}

impl BilibiliDownloader {
    pub fn new(
        room_id: u64,
        quality: u32,
        format: VideoFormat,
        codec: StreamCodec,
        client: HttpClient,
    ) -> Self {
        Self {
            room_id,
            quality,
            format,
            codec,
            client,
            downloader: None,
        }
    }

    /// 获取直播流信息
    async fn get_stream_info(&self) -> Result<LiveRoomStreamUrl> {
        let mut retry_count = 0;
        let max_retries = 5;

        loop {
            match self
                .client
                .get_live_room_stream_url(self.room_id, self.quality)
                .await
            {
                Ok(stream_info) => return Ok(stream_info),
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        anyhow::bail!("获取直播流地址失败，重试次数已达上限: {}", e);
                    }

                    // 指数退避重试
                    let delay = Duration::from_secs(2_u64.pow(retry_count as u32));
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

        // 优先尝试http_stream协议
        if let Some(stream) = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == "http_stream")
        {
            return self.parse_http_stream(stream);
        }

        // 如果没有http_stream，尝试HLS协议
        if let Some(stream) = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == "http_hls")
        {
            return self.parse_hls_stream(stream);
        }

        anyhow::bail!("未找到合适的直播流协议");
    }

    fn parse_http_stream(
        &self,
        stream: &crate::api::stream::PlayStream,
    ) -> Result<(String, DownloaderType)> {
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

        // 创建HttpStreamDownloader（占位符，实际创建在start_download中）
        let config = DownloadConfig {
            output_path: String::new(), // 将在start_download中设置
            overwrite: false,
            timeout: 30,
            retry_count: 3,
        };
        let http_downloader = HttpStreamDownloader::new(url.clone(), config, self.client.clone());

        Ok((url, DownloaderType::HttpStream(http_downloader)))
    }

    fn parse_hls_stream(
        &self,
        stream: &crate::api::stream::PlayStream,
    ) -> Result<(String, DownloaderType)> {
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

        // 创建HttpHlsDownloader（占位符，实际创建在start_download中）
        let config = DownloadConfig {
            output_path: String::new(), // 将在start_download中设置
            overwrite: false,
            timeout: 30,
            retry_count: 3,
        };
        let hls_downloader = HttpHlsDownloader::new(url.clone(), config, self.client.clone());

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
        let ext = self.format.ext(&self.codec);

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
                };
                DownloaderType::HttpStream(HttpStreamDownloader::new(
                    url,
                    config,
                    self.client.clone(),
                ))
            }
            DownloaderType::HttpHls(_) => {
                let config = DownloadConfig {
                    output_path: file_path,
                    overwrite: false,
                    timeout: 30,
                    retry_count: 3,
                };
                DownloaderType::HttpHls(HttpHlsDownloader::new(url, config, self.client.clone()))
            }
        };

        // 开始下载
        match &mut final_downloader {
            DownloaderType::HttpStream(downloader) => {
                downloader.start(cx)?;
            }
            DownloaderType::HttpHls(downloader) => {
                downloader.start(cx)?;
            }
        }

        // 保存下载器引用
        self.downloader = Some(final_downloader);

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
