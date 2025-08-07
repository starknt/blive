use std::{
    collections::VecDeque,
    sync::{Arc, atomic},
    time::Duration,
};

use gpui::{AsyncApp, WeakEntity};
use try_lock::TryLock;

use crate::{
    components::{RoomCard, RoomCardStatus},
    core::{
        DownloadStatus, HttpClient,
        downloader::{DownloadEvent, DownloadStats, utils},
        http_client::{room::LiveRoomInfoData, user::LiveUserInfo},
    },
    log_recording_error, log_recording_start, log_recording_stop,
    settings::{Quality, Strategy, StreamCodec, VideoContainer},
};

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
    /// 下载策略
    pub strategy: Strategy,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            strategy: Strategy::default(),
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

#[derive(Clone)]
pub struct DownloaderContext {
    status: Arc<TryLock<DownloadStatus>>,
    pub entity: WeakEntity<RoomCard>,
    pub client: HttpClient,
    pub room_info: LiveRoomInfoData,
    pub user_info: LiveUserInfo,
    pub quality: Quality,
    pub format: VideoContainer,
    pub codec: StreamCodec,
    pub strategy: Strategy,
    stats: Arc<TryLock<DownloadStats>>,
    is_running: Arc<atomic::AtomicBool>,
    event_queue: Arc<TryLock<VecDeque<DownloadEvent>>>,
}

impl DownloaderContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entity: WeakEntity<RoomCard>,
        client: HttpClient,
        room_info: LiveRoomInfoData,
        user_info: LiveUserInfo,
        strategy: Strategy,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
    ) -> Self {
        Self {
            status: Arc::new(TryLock::new(DownloadStatus::NotStarted)),
            entity,
            client,
            room_info,
            user_info,
            strategy,
            quality,
            format,
            codec,
            stats: Arc::new(TryLock::new(DownloadStats::default())),
            is_running: Arc::new(atomic::AtomicBool::new(false)),
            event_queue: Arc::new(TryLock::new(VecDeque::new())),
        }
    }

    pub fn init(&self) {
        self.stats.try_lock().unwrap().reset();
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.event_queue.try_lock().unwrap().clear();
        self.set_status(DownloadStatus::NotStarted);
    }

    pub fn set_status(&self, status: DownloadStatus) {
        if let Some(mut status_guard) = self.status.try_lock() {
            *status_guard = status;
        }
    }

    pub fn get_status(&self) -> DownloadStatus {
        self.status.try_lock().unwrap().clone()
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
        if let Some(mut queue) = self.event_queue.try_lock() {
            queue.push_back(event);
        }
    }

    /// 处理队列中的所有事件，返回处理的事件数量
    pub fn process_events(&self, cx: &mut AsyncApp) -> usize {
        let mut processed = 0;

        if let Some(mut queue) = self.event_queue.try_lock() {
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

        if !self.is_running() {
            return;
        }

        // 更新UI状态并处理下载器状态
        match &event {
            DownloadEvent::Started { .. } => {
                self.update_card_status(cx, RoomCardStatus::Recording(0.0));
                // 确保运行状态为true
                self.set_running(true);
            }
            DownloadEvent::Progress {
                download_speed_kbps,
                ..
            } => {
                self.update_card_status(cx, RoomCardStatus::Recording(*download_speed_kbps));
                // 更新统计信息
                self.update_stats(|stats| {
                    stats.download_speed_kbps = *download_speed_kbps;
                });
            }
            DownloadEvent::Error { error } => {
                let status = if error.is_recoverable() {
                    RoomCardStatus::Error(format!("网络异常，正在重连: {error}"))
                } else {
                    // 如果是不可恢复的错误，停止下载器, 等待重新连接
                    self.set_status(DownloadStatus::Error(error.clone()));
                    RoomCardStatus::Error(format!("录制失败, 等待重新连接: {error}"))
                };
                self.update_card_status(cx, status);

                // 更新错误统计
                self.update_stats(|stats| {
                    stats.last_error = Some(error.to_string());
                });
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

                // 更新重连统计
                self.update_stats(|stats| {
                    stats.reconnect_count = *attempt;
                });

                // 重连期间保持运行状态
                // self.set_running(true);
            }
            DownloadEvent::Completed { file_size, .. } => {
                self.update_card_status(cx, RoomCardStatus::Waiting);

                // 更新完成统计
                self.update_stats(|stats| {
                    stats.bytes_downloaded = *file_size;
                });

                // 下载完成，停止运行状态
                self.set_running(false);
            }
        }
    }

    /// 记录事件日志
    fn log_event(&self, event: &DownloadEvent) {
        match event {
            DownloadEvent::Started { file_path } => {
                log_recording_start(
                    self.room_info.room_id,
                    &self.quality.to_string(),
                    &format!("文件: {file_path}"),
                );
            }
            DownloadEvent::Progress {
                bytes_downloaded,
                download_speed_kbps,
                duration_ms,
            } => {
                // 只在调试模式下记录详细进度，避免日志过多
                #[cfg(debug_assertions)]
                tracing::debug!(
                    "录制进度 - 房间: {}, 已下载: {:.2}MB, 速度: {:.1}kb/s, 时长: {}秒",
                    self.room_info.room_id,
                    utils::pretty_bytes(*bytes_downloaded),
                    *download_speed_kbps,
                    duration_ms / 1000
                );
            }
            DownloadEvent::Error { error } => {
                if error.is_recoverable() {
                    log_recording_error(
                        self.room_info.room_id,
                        &format!("网络异常，正在重连: {error}"),
                    );
                } else {
                    log_recording_error(self.room_info.room_id, &format!("录制失败: {error}"));
                }
            }
            DownloadEvent::Reconnecting {
                attempt,
                delay_secs,
            } => {
                log_recording_error(
                    self.room_info.room_id,
                    &format!("网络中断，第{attempt}次重连 ({delay_secs}秒后)"),
                );
            }
            DownloadEvent::Completed {
                file_path,
                file_size,
            } => {
                let mb_size = *file_size as f64 / 1024.0 / 1024.0;
                log_recording_stop(self.room_info.room_id);
                tracing::info!(
                    "录制完成 - 房间: {}, 文件: {}, 大小: {:.2}MB",
                    self.room_info.room_id,
                    file_path,
                    mb_size
                );
            }
        }
    }

    /// 启动事件处理任务
    pub fn start_event_processor(&self, cx: &mut AsyncApp) {
        let context = self.clone();

        cx.spawn(async move |cx| {
            while context.is_running() {
                // 每 1s 处理一次事件队列
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
        if let Some(mut stats) = self.stats.try_lock() {
            updater(&mut stats);
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> DownloadStats {
        self.stats
            .try_lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|| {
                eprintln!("无法获取统计信息锁");
                DownloadStats::default()
            })
    }
}
