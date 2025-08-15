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
    settings::RoomSettings,
    state::{AppState, RoomCardState},
};
use gpui::{
    App, ClickEvent, Entity, EntityId, EventEmitter, ObjectFit, SharedString, Subscription,
    WeakEntity, Window, div, img, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, ContextModal, Disableable, Icon, IconName, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    skeleton::Skeleton,
    v_flex,
};
use std::{path::Path, sync::Arc, time::Duration};

#[derive(Clone, Debug)]
pub enum RoomCardEvent {
    LiveStatusChanged(LiveStatus),
    StartRecording(bool),
    StopRecording(bool),
    WillDeleted(u64),
    Deleted(EntityId),
}

#[derive(Clone, Default, PartialEq, Debug)]
pub enum RoomCardStatus {
    #[default]
    WaitLiveStreaming,
    LiveRecording,
}

#[derive(Clone, PartialEq, Debug)]
pub enum DownloaderStatus {
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

pub struct RoomCard {
    client: HttpClient,
    settings: RoomSettings,
    pub settings_modal: Entity<RoomSettingsModal>,
    pub(crate) room_info: Option<LiveRoomInfoData>,
    pub(crate) user_info: Option<LiveUserInfo>,
    pub downloader_speed: Option<f32>,
    pub downloader: Option<Arc<BLiveDownloader>>,
    _subscriptions: Vec<Subscription>,
}

impl RoomCard {
    fn new(
        client: HttpClient,
        settings: RoomSettings,
        settings_modal: Entity<RoomSettingsModal>,
        subscriptions: Vec<Subscription>,
        downloader: Option<Arc<BLiveDownloader>>,
    ) -> Self {
        Self {
            client,
            settings,
            settings_modal,
            room_info: None,
            user_info: None,
            downloader_speed: None,
            downloader,
            _subscriptions: subscriptions,
        }
    }

    pub fn view(
        settings: RoomSettings,
        client: HttpClient,
        downloader: Option<Arc<BLiveDownloader>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let room_id = settings.room_id;

        let task_client = client.clone();
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

        let settings_modal = RoomSettingsModal::view(settings.clone(), window, cx);

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
                                        room.codec =
                                            Some(settings.codec.unwrap_or(global_settings.codec));
                                    }

                                    if settings.format.unwrap_or(global_settings.format)
                                        == global_settings.format
                                    {
                                        room.format = None;
                                    } else {
                                        room.format =
                                            Some(settings.format.unwrap_or(global_settings.format));
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
                                            settings.strategy.unwrap_or(global_settings.strategy),
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

        if let Some(downloader) = downloader.clone() {
            downloader
                .context
                .update_room_card_entity(cx.entity().downgrade());
        }

        Self::new(client, settings, settings_modal, subscription, downloader)
    }

    // 从全局状态获取房间状态
    fn get_room_state(&self, cx: &App) -> Option<RoomCardState> {
        AppState::global(cx)
            .get_room_state(self.settings.room_id)
            .cloned()
    }

    // 更新全局状态中的房间状态
    fn update_room_state<F>(&self, cx: &mut App, updater: F)
    where
        F: FnOnce(&mut RoomCardState),
    {
        cx.update_global(|state: &mut AppState, _| {
            if let Some(room_state) = state.get_room_state_mut(self.settings.room_id) {
                updater(room_state);
            }
        });
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
                    self.update_room_state(cx, |state| {
                        state.user_stop = false;
                    });
                }

                let room_state = self.get_room_state(cx).unwrap();

                if room_state.user_stop || room_state.downloader.is_some() {
                    return;
                }

                let live_status: LiveStatus = match &self.room_info {
                    Some(room_info) => room_info.live_status,
                    None => LiveStatus::Offline,
                };

                match live_status {
                    LiveStatus::Live => {
                        self.update_room_state(cx, |state| {
                            state.status = RoomCardStatus::LiveRecording;
                            state.reconnect_manager.reset_attempts();
                        });

                        let room_info = self.room_info.clone().unwrap_or_default();
                        let user_info = self.user_info.clone().unwrap_or_default();
                        let client = self.client.clone();
                        let setting = self.settings.clone();
                        let room_id = self.settings.room_id;

                        let downloader = Arc::new(BLiveDownloader::new(
                            room_info,
                            user_info,
                            setting.quality.unwrap_or_default(),
                            setting.format.unwrap_or_default(),
                            setting.codec.unwrap_or_default(),
                            setting.strategy.unwrap_or_default(),
                            client,
                            self.settings.room_id,
                        ));

                        downloader.context.update_room_card_entity(this.downgrade());

                        self.downloader = Some(downloader.clone());
                        cx.update_global(|state: &mut AppState, _| {
                            if let Some(room_state) = state.get_room_state_mut(room_id) {
                                room_state.downloader = Some(downloader.clone());
                            }
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
                self.update_room_state(cx, |state| {
                    state.user_stop = *user_action;
                    state.status = RoomCardStatus::WaitLiveStreaming;
                });

                if let Some(downloader) = self.downloader.take() {
                    let room_id = self.settings.room_id;

                    cx.foreground_executor()
                        .spawn(async move {
                            downloader.stop().await;
                        })
                        .detach();

                    cx.update_global(|state: &mut AppState, _| {
                        if let Some(room_state) = state.get_room_state_mut(room_id) {
                            room_state.downloader = None;
                        }
                    });

                    self.downloader = None;
                }

                // 刷新窗口
                cx.refresh_windows();
            }
            RoomCardEvent::WillDeleted(room_id) => {
                cx.emit(RoomCardEvent::Deleted(this.entity_id()));

                cx.update_global(|state: &mut AppState, _| {
                    state.remove_room_state(*room_id);
                    state.settings.rooms.retain(|d| d.room_id != *room_id);
                    log_user_action("房间删除完成", Some(&format!("房间号: {room_id}")));
                });
            }
            _ => {}
        }
    }

    fn on_downloader_event(
        &mut self,
        _: &Entity<Self>,
        event: &DownloaderEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            DownloaderEvent::Started { .. } => {
                self.downloader_speed = None;
            }
            DownloaderEvent::Progress { speed } => {
                self.downloader_speed = Some(*speed);
            }
            DownloaderEvent::Completed { .. } => {
                self.downloader_speed = None;
                cx.emit(RoomCardEvent::StopRecording(false));
            }
            DownloaderEvent::Reconnecting => {
                let state = self.get_room_state(cx).unwrap();
                let room_id = self.settings.room_id;
                if state.reconnect_manager.should_reconnect() {
                    let delay = state.reconnect_manager.calculate_delay();
                    let record_dir = state.settings.record_dir.clone().unwrap_or_default();
                    let downloader = state.downloader.clone().unwrap();

                    cx.spawn(async move |_, cx| {
                        cx.background_executor().timer(delay).await;
                        let _ = downloader.restart(cx, &record_dir).await;
                        let _ = cx.update_global(|state: &mut AppState, _| {
                            state
                                .get_room_state_mut(room_id)
                                .unwrap()
                                .reconnect_manager
                                .increment_attempt();
                        });
                    })
                    .detach();
                }
            }
            DownloaderEvent::Error { .. } => {
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
        let room_state = self.get_room_state(cx).unwrap_or_default().clone();

        div()
            .rounded_lg()
            .p_4()
            .border(px(1.0))
            .border_color(cx.theme().border)
            .when(matches!(room_state.downloader_status, Some(DownloaderStatus::Error { .. })), |div| {
                div.border_color(cx.theme().red)
            })
            .when(room_state.reconnecting, |div| {
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
                                                            room_state.status,
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
                                            room_state.status,
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
                                                    .label(match &room_state.status {
                                                        RoomCardStatus::WaitLiveStreaming => {
                                                            "开始录制"
                                                        }
                                                        RoomCardStatus::LiveRecording => {
                                                            "停止录制"
                                                        }
                                                    })
                                                    .on_click(cx.listener(|card, _, _, cx| {
                                                        let room_id = card.settings.room_id;
                                                        match &card.get_room_state(cx).unwrap().status {
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
                            .when_some(room_state.downloader_status.clone(), |div, status| {
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
