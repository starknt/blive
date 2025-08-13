use crate::{
    components::room_settings_modal::{RoomSettingsModal, RoomSettingsModalEvent},
    core::{
        HttpClient,
        downloader::{
            BLiveDownloader,
            context::DownloaderEvent,
            utils::{pretty_bytes, pretty_duration},
        },
        http_client::{
            room::{LiveRoomInfoData, LiveStatus},
            user::LiveUserInfo,
        },
    },
    logger::log_user_action,
    settings::{GlobalSettings, RoomSettings},
    state::AppState,
};
use gpui::{
    App, ClickEvent, Entity, EventEmitter, ObjectFit, SharedString, Subscription, WeakEntity,
    Window, div, img, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, ContextModal, Disableable, Icon, IconName, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    notification::Notification,
    skeleton::Skeleton,
    v_flex,
};
use rand::Rng;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

#[derive(Clone, Debug)]
enum RoomCardEvent {
    LiveStatusChanged(LiveStatus),
    StartRecording(bool),
    StopRecording(bool),
    WillDeleted(u64),
}

#[derive(Clone, Default, PartialEq, Debug)]
pub enum RoomCardStatus {
    #[default]
    WaitLiveStreaming,
    LiveRecording,
}

#[derive(Clone, PartialEq, Debug)]
enum DownloaderStatus {
    Started {
        file_path: String,
    },
    Completed {
        file_path: String,
        file_size: u64,
        duration: u64,
    },
    Error {
        cause: String,
    },
}

#[derive(Debug)]
struct ReconnectManager {
    current_attempt: u32,
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    last_reconnect_time: Option<std::time::Instant>,
}

impl ReconnectManager {
    fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            current_attempt: 0,
            max_attempts,
            base_delay,
            max_delay,
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

    fn calculate_delay(&self) -> Duration {
        // 指数退避算法，带随机抖动
        let exponential_delay = self.base_delay * (2_u32.pow(self.current_attempt.min(10)));
        let jitter = rand::rng().random_range(0.8..1.2);

        let delay = Duration::from_secs_f64(exponential_delay.as_secs_f64() * jitter);

        delay.min(self.max_delay)
    }
}

pub struct RoomCard {
    client: HttpClient,
    pub settings_modal: Entity<RoomSettingsModal>,
    pub(crate) status: RoomCardStatus,
    pub(crate) room_info: Option<LiveRoomInfoData>,
    pub(crate) user_info: Option<LiveUserInfo>,
    pub(crate) settings: RoomSettings,
    pub user_stop: bool,
    _subscriptions: Vec<Subscription>,
    pub downloader: Option<Arc<BLiveDownloader>>,
    downloader_status: Option<DownloaderStatus>,
    downloader_speed: Option<f32>,
    reconnect_manager: ReconnectManager,
    reconnecting: Arc<AtomicBool>,
}

impl RoomCard {
    fn new(
        client: HttpClient,
        settings: RoomSettings,
        settings_modal: Entity<RoomSettingsModal>,
        subscriptions: Vec<Subscription>,
    ) -> Self {
        Self {
            client,
            settings,
            user_stop: false,
            settings_modal,
            _subscriptions: subscriptions,
            status: RoomCardStatus::default(),
            room_info: None,
            user_info: None,
            downloader: None,
            downloader_status: None,
            downloader_speed: None,
            reconnect_manager: ReconnectManager::new(
                u32::MAX,
                Duration::from_secs(1),
                Duration::from_secs(30),
            ),
            reconnecting: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn view(
        global_settings: GlobalSettings,
        settings: RoomSettings,
        window: &mut Window,
        cx: &mut App,
        client: HttpClient,
    ) -> Entity<Self> {
        let room_id = settings.room_id;

        let task_client = client.clone();
        cx.new(|cx| {
            cx.spawn(async move |this: WeakEntity<RoomCard>, cx| {
                loop {
                    if !this.is_upgradable() {
                        break;
                    }

                    if let Some(this) = this.upgrade() {
                        let (room_data, user_data) = futures::join!(
                            task_client.get_live_room_info(room_id),
                            task_client.get_live_room_user_info(room_id)
                        );

                        match (room_data, user_data) {
                            (Ok(room_info), Ok(user_info)) => {
                                let _ = this.update(cx, |this, cx| {
                                    let now_room_live_status = room_info.live_status;

                                    this.user_info = Some(user_info.info);
                                    this.room_info = Some(room_info);

                                    cx.emit(RoomCardEvent::LiveStatusChanged(now_room_live_status));
                                    cx.notify();
                                });
                            }
                            (Ok(room_info), Err(_)) => {
                                let _ = this.update(cx, |this: &mut RoomCard, cx| {
                                    let now_room_live_status = room_info.live_status;
                                    this.room_info = Some(room_info);

                                    cx.emit(RoomCardEvent::LiveStatusChanged(now_room_live_status));
                                    cx.notify();
                                });
                            }
                            (Err(_), Ok(user_info)) => {
                                let _ = this.update(cx, |this, cx| {
                                    this.user_info = Some(user_info.info);
                                    cx.notify();
                                });
                            }
                            (Err(_), Err(_)) => {
                                // nothing
                            }
                        }
                    }

                    cx.background_executor()
                        .timer(Duration::from_secs(10))
                        .await;
                }
            })
            .detach();

            let settings_modal = RoomSettingsModal::view(
                RoomSettings {
                    room_id,
                    strategy: Some(settings.strategy.unwrap_or(global_settings.strategy)),
                    quality: Some(settings.quality.unwrap_or(global_settings.quality)),
                    format: Some(settings.format.unwrap_or(global_settings.format)),
                    codec: Some(settings.codec.unwrap_or(global_settings.codec)),
                    record_name: settings.record_name.clone(),
                    record_dir: match settings.record_dir.clone().unwrap_or_default().is_empty() {
                        true => Some(global_settings.record_dir.clone()),
                        false => settings.record_dir.clone(),
                    },
                },
                window,
                cx,
            );

            let subscription = vec![
                cx.subscribe_in(
                    &settings_modal,
                    window,
                    |card: &mut RoomCard, _, event, window, cx| match event {
                        RoomSettingsModalEvent::SaveSettings(settings) => {
                            card.settings = settings.clone();

                            cx.update_global(|state: &mut AppState, _| {
                                let global_settings = state.settings.clone();

                                // 更新房间设置
                                for room in state.settings.rooms.iter_mut() {
                                    if room.room_id == settings.room_id {
                                        if settings.codec.unwrap_or(global_settings.codec)
                                            == global_settings.codec
                                        {
                                            room.codec = None;
                                        } else {
                                            room.codec = Some(
                                                settings.codec.unwrap_or(global_settings.codec),
                                            );
                                        }

                                        if settings.format.unwrap_or(global_settings.format)
                                            == global_settings.format
                                        {
                                            room.format = None;
                                        } else {
                                            room.format = Some(
                                                settings.format.unwrap_or(global_settings.format),
                                            );
                                        }

                                        if settings.quality.unwrap_or(global_settings.quality)
                                            == global_settings.quality
                                        {
                                            room.quality = None;
                                        } else {
                                            room.quality = Some(
                                                settings.quality.unwrap_or(global_settings.quality),
                                            );
                                        }

                                        if settings.strategy.unwrap_or(global_settings.strategy)
                                            == global_settings.strategy
                                        {
                                            room.strategy = None;
                                        } else {
                                            room.strategy = Some(
                                                settings
                                                    .strategy
                                                    .unwrap_or(global_settings.strategy),
                                            );
                                        }
                                    }
                                }
                            });
                        }
                        RoomSettingsModalEvent::QuitSettings => {
                            window.close_modal(cx);
                        }
                    },
                ),
                cx.subscribe_in(&cx.entity(), window, Self::on_event),
                cx.subscribe_in(&cx.entity(), window, Self::on_downloader_event),
            ];

            Self::new(
                client,
                RoomSettings {
                    room_id,
                    strategy: Some(settings.strategy.unwrap_or(global_settings.strategy)),
                    quality: Some(settings.quality.unwrap_or(global_settings.quality)),
                    format: Some(settings.format.unwrap_or(global_settings.format)),
                    codec: Some(settings.codec.unwrap_or(global_settings.codec)),
                    record_name: settings.record_name.clone(),
                    record_dir: match settings.record_dir.clone().unwrap_or_default().is_empty() {
                        true => Some(global_settings.record_dir.clone()),
                        false => settings.record_dir.clone(),
                    },
                },
                settings_modal,
                subscription,
            )
        })
    }
}

impl RoomCard {
    fn on_delete(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let room_id = self.settings.room_id;
        log_user_action("删除房间", Some(&format!("房间号: {room_id}")));

        if let Some(downloader) = self.downloader.take() {
            cx.foreground_executor()
                .spawn(async move {
                    downloader.stop().await;
                })
                .detach();
        }

        cx.emit(RoomCardEvent::WillDeleted(room_id));
    }

    fn on_open_settings(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let room_id = self.settings.room_id;
        log_user_action("打开房间设置", Some(&format!("房间号: {room_id}")));

        let setting_modal = self.settings_modal.clone();
        window.open_modal(cx, move |modal, _, _| {
            modal
                .rounded_lg()
                .title(
                    div()
                        .font_bold()
                        .text_2xl()
                        .child("房间设置".into_element()),
                )
                .overlay_closable(false)
                .child(setting_modal.clone())
        });
    }

    fn on_event(
        &mut self,
        this: &Entity<Self>,
        event: &RoomCardEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            RoomCardEvent::LiveStatusChanged(status) => {
                match status {
                    LiveStatus::Live => {
                        cx.emit(RoomCardEvent::StartRecording(false));
                    }
                    LiveStatus::Carousel | LiveStatus::Offline => {
                        cx.emit(RoomCardEvent::StopRecording(false))
                    }
                }

                cx.notify();
            }
            RoomCardEvent::StartRecording(user_action) => {
                if *user_action {
                    self.user_stop = false;
                }

                if self.user_stop || self.downloader.is_some() {
                    return;
                }

                let live_status: LiveStatus = match &self.room_info {
                    Some(room_info) => room_info.live_status,
                    None => LiveStatus::Offline,
                };

                match live_status {
                    LiveStatus::Live => {
                        self.downloader_speed = None;
                        self.downloader_status = None;
                        self.status = RoomCardStatus::LiveRecording;

                        let room_info = self.room_info.clone().unwrap_or_default();
                        let user_info = self.user_info.clone().unwrap_or_default();
                        let client = self.client.clone();
                        let setting = self.settings.clone();

                        let downloader = Arc::new(BLiveDownloader::new(
                            room_info,
                            user_info,
                            setting.quality.unwrap_or_default(),
                            setting.format.unwrap_or_default(),
                            setting.codec.unwrap_or_default(),
                            setting.strategy.unwrap_or_default(),
                            client,
                            this.downgrade(),
                        ));

                        self.downloader = Some(downloader.clone());
                        cx.update_global(|state: &mut AppState, _| {
                            state.downloaders.push(downloader.clone());
                        });

                        cx.spawn(async move |_, cx| {
                            match downloader
                                .start(cx, &setting.record_dir.unwrap_or_default())
                                .await
                            {
                                Ok(_) => {
                                    // 下载成功完成，状态会通过事件回调自动更新
                                }
                                Err(e) => {
                                    // 错误也会通过事件回调处理，但这里我们可以做额外的日志记录
                                    eprintln!("下载器启动失败: {e}");
                                }
                            }
                        })
                        .detach();
                    }
                    LiveStatus::Carousel | LiveStatus::Offline => {
                        cx.emit(RoomCardEvent::StopRecording(false));
                    }
                }

                cx.notify();
            }
            RoomCardEvent::StopRecording(user_action) => {
                self.user_stop = *user_action;
                self.downloader_speed = None;
                self.status = RoomCardStatus::WaitLiveStreaming;

                if let Some(downloader) = self.downloader.take() {
                    let room_id = self.settings.room_id;

                    cx.foreground_executor()
                        .spawn(async move {
                            downloader.stop().await;
                        })
                        .detach();

                    cx.update_global(|state: &mut AppState, _| {
                        state
                            .downloaders
                            .retain(|d| d.context.room_info.room_id != room_id);
                    });

                    self.downloader = None;
                }

                // 刷新窗口
                cx.refresh_windows();
            }
            RoomCardEvent::WillDeleted(room_id) => {
                cx.update_global(|state: &mut AppState, _| {
                    state
                        .downloaders
                        .retain(|d| d.context.room_info.room_id != *room_id);
                    state
                        .room_entities
                        .retain(|e| e.entity_id() != this.entity_id());
                    state.settings.rooms.retain(|d| d.room_id != *room_id);
                    log_user_action("房间删除完成", Some(&format!("房间号: {room_id}")));
                });
            }
        }
    }

    fn on_downloader_event(
        &mut self,
        _: &Entity<Self>,
        event: &DownloaderEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            DownloaderEvent::Started { file_path } => {
                self.downloader_status = Some(DownloaderStatus::Started {
                    file_path: file_path.to_owned(),
                });
                self.downloader_speed = None;
                window
                    .push_notification(Notification::success(format!("开始录制 {file_path}")), cx);
            }
            DownloaderEvent::Progress { speed } => {
                self.downloader_speed = Some(*speed);
            }
            DownloaderEvent::Completed {
                file_path,
                file_size,
                duration,
            } => {
                self.downloader_status = Some(DownloaderStatus::Completed {
                    file_path: file_path.to_owned(),
                    file_size: *file_size,
                    duration: *duration,
                });
                self.downloader_speed = None;
                cx.emit(RoomCardEvent::StopRecording(false));
            }
            DownloaderEvent::Reconnecting => {
                if self.reconnecting.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }

                if self.reconnect_manager.should_reconnect() {
                    let reconnecting = self.reconnecting.clone();
                    self.reconnect_manager.increment_attempt();
                    let delay = self.reconnect_manager.calculate_delay();
                    let record_dir = self.settings.record_dir.clone().unwrap_or_default();
                    let downloader = self.downloader.clone().unwrap();

                    cx.spawn(async move |_, cx| {
                        reconnecting.store(true, std::sync::atomic::Ordering::Relaxed);
                        cx.background_executor().timer(delay).await;
                        let _ = downloader.restart(cx, &record_dir).await;
                        reconnecting.store(false, std::sync::atomic::Ordering::Relaxed);
                    })
                    .detach();
                }
            }
            DownloaderEvent::Error { cause } => {
                self.downloader_status = Some(DownloaderStatus::Error {
                    cause: cause.to_owned(),
                });
                self.downloader_speed = None;
            }
        }

        cx.notify();
    }
}

impl EventEmitter<RoomCardEvent> for RoomCard {}

impl EventEmitter<DownloaderEvent> for RoomCard {}

impl Render for RoomCard {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let room_info = &self.room_info;
        let user_info = &self.user_info;

        if room_info.is_none() || user_info.is_none() {
            return v_flex()
                .rounded_lg()
                .p_4()
                .gap_y_2()
                .border(px(1.0))
                .border_color(cx.theme().border)
                .child(
                    v_flex().gap_4().child(
                        h_flex()
                            .justify_between()
                            .items_start()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_start()
                                    .child(
                                        Skeleton::new()
                                            .rounded_lg()
                                            .w_40()
                                            .p_4()
                                            .border(px(1.0))
                                            .border_color(cx.theme().border)
                                            .h_full(),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                Skeleton::new()
                                                    .rounded_lg()
                                                    .p_4()
                                                    .border(px(1.0))
                                                    .border_color(cx.theme().border)
                                                    .size_full()
                                                    .w_56(),
                                            )
                                            .child(
                                                Skeleton::new()
                                                    .rounded_lg()
                                                    .p_4()
                                                    .border(px(1.0))
                                                    .border_color(cx.theme().border)
                                                    .size_full()
                                                    .w_32(),
                                            )
                                            .child(
                                                Skeleton::new()
                                                    .rounded_lg()
                                                    .p_4()
                                                    .border(px(1.0))
                                                    .border_color(cx.theme().border)
                                                    .size_full()
                                                    .w_24(),
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Skeleton::new()
                                            .rounded_lg()
                                            .p_4()
                                            .border(px(1.0))
                                            .border_color(cx.theme().border)
                                            .size_full()
                                            .w_32(),
                                    )
                                    .child(
                                        Skeleton::new()
                                            .rounded_lg()
                                            .p_4()
                                            .border(px(1.0))
                                            .border_color(cx.theme().border)
                                            .size_full()
                                            .w_32(),
                                    )
                                    .child(
                                        Skeleton::new()
                                            .rounded_lg()
                                            .p_4()
                                            .border(px(1.0))
                                            .border_color(cx.theme().border)
                                            .size_full()
                                            .w_16(),
                                    ),
                            ),
                    ),
                )
                .child(
                    Skeleton::new()
                        .rounded_lg()
                        .p_4()
                        .border(px(1.0))
                        .border_color(cx.theme().border)
                        .size_full()
                        .w_112(),
                );
        }

        let room_info = room_info.clone().unwrap_or_default();
        let user_info = user_info.clone().unwrap_or_default();

        div()
            .rounded_lg()
            .p_4()
            .border(px(1.0))
            .border_color(cx.theme().border)
            .when(matches!(self.downloader_status, Some(DownloaderStatus::Error { .. })), |div| {
                div.border_color(cx.theme().red)
            })
            .when(self.reconnecting.load(std::sync::atomic::Ordering::Relaxed), |div| {
                div.border_color(cx.theme().warning)
            })
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        // 房间头部信息
                        h_flex()
                            .justify_between()
                            .items_start()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_start()
                                    .child(
                                        div().w_40().child(
                                            div()
                                                .rounded(cx.theme().radius_lg)
                                                .overflow_hidden()
                                                .size_full()
                                                .child(
                                                    img(room_info.user_cover.clone())
                                                        .block()
                                                        .size_full()
                                                        .rounded(cx.theme().radius_lg)
                                                        .overflow_hidden()
                                                        .object_fit(ObjectFit::Cover),
                                                ),
                                        ),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(room_info.title.clone().into_element())
                                                    .child(div().font_bold().child(
                                                        user_info.uname.clone().into_element(),
                                                    )),
                                            )
                                            .child(
                                                format!(
                                                    "房间号: {}",
                                                    if room_info.short_id > 0 {
                                                        room_info.short_id
                                                    } else {
                                                        room_info.room_id
                                                    }
                                                )
                                                .into_element(),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(div().w_2().h_2().rounded_full().bg(
                                                        match room_info.live_status {
                                                            LiveStatus::Live => gpui::rgb(0xef4444),
                                                            _ => gpui::rgb(0x6b7280),
                                                        },
                                                    ))
                                                    .child(
                                                        match room_info.live_status {
                                                            LiveStatus::Live => "直播中".into_element(),
                                                            LiveStatus::Carousel => "轮播中".into_element(),
                                                            LiveStatus::Offline => "未开播".into_element(),
                                                        }
                                                    )
                                                    .when(
                                                        matches!(
                                                            self.status,
                                                            RoomCardStatus::LiveRecording
                                                        ),
                                                        |div| {
                                                            div.child(
                                                                format!(
                                                                    "{} 人观看",
                                                                    room_info.online
                                                                )
                                                                .into_element(),
                                                            )
                                                        },
                                                    )
                                                    .when(matches!(room_info.live_status, LiveStatus::Live), |div| div.child(format!("分区: {}", room_info.area_name).into_element()))
                                                    .when(matches!(room_info.live_status, LiveStatus::Live), |div| div.child(format!("开始时间: {}", room_info.live_time).into_element()))
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .when(
                                        matches!(
                                            self.status,
                                            RoomCardStatus::LiveRecording
                                                | RoomCardStatus::WaitLiveStreaming
                                        ),
                                        |div| {
                                            div.child(h_flex().flex_1().children(vec![
                                                Button::new("record")
                                                    .primary()
                                                    .map(|this| {
                                                        let play_icon = Icon::default();
                                                        let play_icon = play_icon.path(
                                                            SharedString::new("icons/play.svg"),
                                                        );
                                                        let pause_icon = Icon::default();
                                                        let pause_icon = pause_icon.path(
                                                            SharedString::new("icons/pause.svg"),
                                                        );

                                                        if matches!(
                                                            room_info.live_status,
                                                            LiveStatus::Live
                                                        ) {
                                                            this.icon(pause_icon)
                                                        } else {
                                                            this.icon(play_icon)
                                                        }
                                                    })
                                                    .disabled(!matches!(
                                                        room_info.live_status,
                                                        LiveStatus::Live
                                                    ))
                                                    .label(match &self.status {
                                                        RoomCardStatus::WaitLiveStreaming => {
                                                            "开始录制"
                                                        }
                                                        RoomCardStatus::LiveRecording => {
                                                            "停止录制"
                                                        }
                                                    })
                                                    .on_click(cx.listener(|card, _, _, cx| {
                                                        let room_id = card.settings.room_id;
                                                        match &card.status {
                                                            RoomCardStatus::WaitLiveStreaming => {
                                                                log_user_action(
                                                                    "开始录制",
                                                                    Some(&format!(
                                                                        "房间号: {room_id}"
                                                                    )),
                                                                );

                                                                cx.emit(RoomCardEvent::StartRecording(true));
                                                            }
                                                            RoomCardStatus::LiveRecording => {
                                                                log_user_action(
                                                                    "停止录制",
                                                                    Some(&format!(
                                                                        "房间号: {room_id}"
                                                                    )),
                                                                );

                                                                cx.emit(RoomCardEvent::StopRecording(true));
                                                            }
                                                        };
                                                    })),
                                            ]))
                                        },
                                    )
                                    .child(
                                        Button::new("settings")
                                            .primary()
                                            .icon(IconName::Settings2)
                                            .label("房间设置")
                                            .on_click(cx.listener(Self::on_open_settings)),
                                    )
                                    .child(
                                        Button::new("删除")
                                            .danger()
                                            .map(|this| {
                                                let icon = Icon::default();
                                                let icon =
                                                    icon.path(SharedString::new("icons/trash.svg"));
                                                this.icon(icon)
                                            })
                                            .label("删除")
                                            .on_click(cx.listener(Self::on_delete)),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_x_4()
                            .items_center()
                            .when_some(self.downloader_status.clone(), |div, status| {
                                match status {
                                    DownloaderStatus::Started { ref file_path } => {
                                        div.child(format!("录制中: {}", Path::new(file_path).file_name().unwrap_or_default().to_string_lossy()).into_element())
                                    }
                                    DownloaderStatus::Completed { ref file_path, ref file_size, ref duration } => {
                                        div.child(format!("录制完成: {} 大小: {} 时长: {}", file_path, pretty_bytes(*file_size), pretty_duration(*duration)).into_element())
                                    }
                                    DownloaderStatus::Error { ref cause } => {
                                        div.child(format!("录制失败: {}", cause).into_element())
                                    }
                                }
                            })
                            .when_some(self.downloader_speed, |div, speed| {
                                div.child(format!("{speed:.2} Kb/s").into_element())
                            })
                    ),
            )
    }
}
