use crate::{
    components::room_settings_modal::{RoomSettingsModal, RoomSettingsModalEvent},
    core::{
        HttpClient,
        downloader::{BLiveDownloader, utils::pretty_bytes},
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
    App, ClickEvent, Entity, EventEmitter, ObjectFit, Subscription, WeakEntity, Window, div, img,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, ContextModal, Disableable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    text::Text,
    v_flex,
};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Debug)]
enum RoomCardEvent {
    LiveStatusChanged(LiveStatus),
    StatusChanged(RoomCardStatus),
    StartRecording,
    StopRecording,
    WillDeleted(u64),
    SettingsChanged(RoomSettings),
    QuitSettings,
}

#[derive(Clone, Default, PartialEq, Debug)]
pub enum RoomCardStatus {
    #[default]
    Waiting,
    Recording(f32),
    Error(String),
}

impl std::fmt::Display for RoomCardStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoomCardStatus::Waiting => write!(f, "等待中"),
            RoomCardStatus::Recording(speed) => {
                write!(f, "录制中: {}/s", pretty_bytes((speed * 1024.0) as u64))
            }
            RoomCardStatus::Error(err) => write!(f, "错误: {err}"),
        }
    }
}

pub struct RoomCard {
    client: HttpClient,
    pub(crate) status: RoomCardStatus,
    pub(crate) room_info: Option<LiveRoomInfoData>,
    pub(crate) user_info: Option<LiveUserInfo>,
    pub(crate) settings: RoomSettings,
    pub downloader: Option<Arc<BLiveDownloader>>,
    pub user_stop: bool,
    pub settings_modal: Entity<RoomSettingsModal>,
    _subscriptions: Vec<Subscription>,
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
                cx.background_executor().timer(Duration::from_secs(2)).await;

                while let Some(this) = this.upgrade() {
                    let (room_data, user_data) = futures::join!(
                        task_client.get_live_room_info(room_id),
                        task_client.get_live_room_user_info(room_id)
                    );

                    match (room_data, user_data) {
                        (Ok(room_info), Ok(user_info)) => {
                            let _ = this.update(cx, |this, cx| {
                                let card_room_live_status = this
                                    .room_info
                                    .as_ref()
                                    .map_or(LiveStatus::Offline, |room_info| room_info.live_status);
                                let now_room_live_status = room_info.live_status;

                                this.user_info = Some(user_info.info);
                                this.room_info = Some(room_info);

                                if now_room_live_status != card_room_live_status {
                                    cx.emit(RoomCardEvent::LiveStatusChanged(now_room_live_status));
                                }
                                cx.notify();
                            });
                        }
                        (Ok(room_info), Err(_)) => {
                            let _ = this.update(cx, |this: &mut RoomCard, cx| {
                                let card_room_live_status = this
                                    .room_info
                                    .as_ref()
                                    .map_or(LiveStatus::Offline, |room_info| room_info.live_status);
                                let now_room_live_status = room_info.live_status;
                                this.room_info = Some(room_info);

                                if now_room_live_status != card_room_live_status {
                                    cx.emit(RoomCardEvent::LiveStatusChanged(now_room_live_status));
                                }
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

                    cx.background_executor()
                        .timer(Duration::from_secs(15))
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
                    record_dir: settings.record_dir.clone(),
                },
                window,
                cx,
            );

            let subscription = vec![
                cx.subscribe(
                    &settings_modal,
                    |card: &mut RoomCard, _, event, cx| match event {
                        RoomSettingsModalEvent::SaveSettings(settings) => {
                            card.settings = settings.clone();
                            cx.emit(RoomCardEvent::SettingsChanged(settings.clone()));
                        }
                        RoomSettingsModalEvent::QuitSettings => {
                            cx.emit(RoomCardEvent::QuitSettings);
                        }
                    },
                ),
                cx.subscribe(&cx.entity(), Self::on_event),
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
                    record_dir: settings.record_dir.clone(),
                },
                settings_modal,
                subscription,
            )
        })
    }

    fn on_event(&mut self, this: Entity<Self>, event: &RoomCardEvent, cx: &mut Context<Self>) {
        match event {
            RoomCardEvent::LiveStatusChanged(status) => {
                self.status = match status {
                    LiveStatus::Live => RoomCardStatus::Recording(0.0),
                    _ => RoomCardStatus::Waiting,
                };
                cx.emit(RoomCardEvent::StatusChanged(self.status.clone()));
            }
            RoomCardEvent::StartRecording => {
                this.update(cx, |this, cx| {
                    this.user_stop = false;

                    let live_status = match &this.room_info {
                        Some(room_info) => room_info.live_status,
                        None => LiveStatus::Offline,
                    };

                    match live_status {
                        LiveStatus::Live => {
                            if this.status != RoomCardStatus::Waiting {
                                // 如果已经在录制中，则不重复开始
                                return;
                            }

                            this.status = RoomCardStatus::Recording(0.0);
                            cx.emit(RoomCardEvent::StatusChanged(this.status.clone()));
                        }
                        _ => {
                            this.status = RoomCardStatus::Error("房间未开播".to_string());
                        }
                    }

                    cx.notify();
                });
            }
            RoomCardEvent::StopRecording => {
                this.update(cx, |this, cx| {
                    let downloader = this.downloader.take();
                    if downloader.is_some() {
                        cx.foreground_executor()
                            .spawn(async move {
                                if let Some(downloader) = downloader {
                                    downloader.stop().await;
                                }
                            })
                            .detach();

                        let room_id = this.settings.room_id;

                        cx.update_global(|state: &mut AppState, _| {
                            state
                                .downloaders
                                .retain(|d| d.context.room_info.room_id != room_id);
                        });
                    }

                    this.status = RoomCardStatus::Waiting;
                    this.user_stop = true;
                    cx.notify();
                });
            }
            RoomCardEvent::StatusChanged(status) => {
                match status {
                    RoomCardStatus::Recording(_speed) => {
                        if !self.user_stop {
                            let room_info = self.room_info.clone();
                            let user_info = self.user_info.clone();
                            let client = self.client.clone();
                            let setting = self.settings.clone();

                            cx.spawn(async move |card, cx| {
                                let downloader = Arc::new(BLiveDownloader::new(
                                    room_info.unwrap(),
                                    user_info.unwrap(),
                                    setting.quality.unwrap_or_default(),
                                    setting.format.unwrap_or_default(),
                                    setting.codec.unwrap_or_default(),
                                    setting.strategy.unwrap_or_default(),
                                    client,
                                    card,
                                ));

                                let _ = this.update(cx, |this, _| {
                                    this.downloader = Some(downloader.clone());
                                });

                                let _ = cx.update_global(|state: &mut AppState, _| {
                                    state.downloaders.push(downloader.clone());
                                });

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
                    }
                    RoomCardStatus::Waiting => {
                        this.update(cx, |this, cx| {
                            this.status = RoomCardStatus::Waiting;
                            let downloader = this.downloader.take();
                            if downloader.is_some() {
                                cx.foreground_executor()
                                    .spawn(async move {
                                        if let Some(downloader) = downloader {
                                            downloader.stop().await;
                                        }
                                    })
                                    .detach();

                                let room_id = this.settings.room_id;
                                cx.update_global(|state: &mut AppState, _| {
                                    state
                                        .downloaders
                                        .retain(|d| d.context.room_info.room_id != room_id);
                                });
                            }

                            cx.notify();
                        });
                    }
                    RoomCardStatus::Error(_err) => {
                        // 错误
                    }
                }
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
            RoomCardEvent::SettingsChanged(settings) => {
                self.settings = settings.clone();
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
                                room.codec = Some(settings.codec.unwrap_or(global_settings.codec));
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
                                room.quality =
                                    Some(settings.quality.unwrap_or(global_settings.quality));
                            }

                            if settings.strategy.unwrap_or(global_settings.strategy)
                                == global_settings.strategy
                            {
                                room.strategy = None;
                            } else {
                                room.strategy =
                                    Some(settings.strategy.unwrap_or(global_settings.strategy));
                            }
                        }
                    }
                });
            }
            RoomCardEvent::QuitSettings => {
                if let Some(window) = cx.active_window() {
                    let _ = cx.update_window(window, |_, window, cx| {
                        window.close_modal(cx);
                    });
                }
            }
        }
    }

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
}

impl RoomCard {
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
                        .child(Text::String("房间设置".into())),
                )
                .overlay_closable(false)
                .child(setting_modal.clone())
        });
    }
}

impl EventEmitter<RoomCardEvent> for RoomCard {}

impl Render for RoomCard {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let room_info = &self.room_info;
        let user_info = &self.user_info;

        if room_info.is_none() || user_info.is_none() {
            return div()
                .rounded_lg()
                .p_4()
                .border(px(1.0))
                .border_color(cx.theme().border)
                .child(Text::String("房间信息加载中...".into()));
        }

        let room_info = room_info.clone().unwrap();
        let user_info = user_info.clone().unwrap();

        div()
            .rounded_lg()
            .p_4()
            .border(px(1.0))
            .border_color(match self.status {
                RoomCardStatus::Error(_) => gpui::rgb(0xef4444),
                _ => cx.theme().border.into(),
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
                                                    .child(div().font_bold().child(Text::String(
                                                        user_info.uname.clone().into(),
                                                    ))),
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
                                                    .child(Text::String(
                                                        match room_info.live_status {
                                                            LiveStatus::Live => "直播中".into(),
                                                            LiveStatus::Carousel => "轮播中".into(),
                                                            LiveStatus::Offline => "未开播".into(),
                                                        },
                                                    ))
                                                    .when(
                                                        matches!(
                                                            self.status,
                                                            RoomCardStatus::Recording(_)
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
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child({
                                        if matches!(
                                            self.status,
                                            RoomCardStatus::Recording(_) | RoomCardStatus::Waiting
                                        ) {
                                            h_flex().flex_1().children(vec![
                                                Button::new("record")
                                                    .primary()
                                                    .disabled(!matches!(
                                                        room_info.live_status,
                                                        LiveStatus::Live
                                                    ))
                                                    .label(match &self.status {
                                                        RoomCardStatus::Waiting => {
                                                            "开始录制".into()
                                                        }
                                                        RoomCardStatus::Recording(_) => {
                                                            "停止录制".into()
                                                        }
                                                        RoomCardStatus::Error(err) => {
                                                            format!("错误: {err}")
                                                        }
                                                    })
                                                    .on_click(cx.listener(|card, _, _, cx| {
                                                        let room_id = card.settings.room_id;
                                                        match &card.status {
                                                            RoomCardStatus::Waiting => {
                                                                log_user_action(
                                                                    "开始录制",
                                                                    Some(&format!(
                                                                        "房间号: {room_id}"
                                                                    )),
                                                                );
                                                                cx.emit(
                                                                    RoomCardEvent::StartRecording,
                                                                );
                                                            }
                                                            RoomCardStatus::Recording(_) => {
                                                                log_user_action(
                                                                    "停止录制",
                                                                    Some(&format!(
                                                                        "房间号: {room_id}"
                                                                    )),
                                                                );
                                                                cx.emit(
                                                                    RoomCardEvent::StopRecording,
                                                                );
                                                            }
                                                            RoomCardStatus::Error(_) => {
                                                                log_user_action(
                                                                    "重试录制",
                                                                    Some(&format!(
                                                                        "房间号: {room_id}"
                                                                    )),
                                                                );
                                                            }
                                                        };
                                                        cx.notify();
                                                    })),
                                            ])
                                        } else {
                                            h_flex().flex_1()
                                        }
                                    })
                                    .child(
                                        Button::new("settings")
                                            .primary()
                                            .label("房间设置")
                                            .on_click(cx.listener(Self::on_open_settings)),
                                    )
                                    .child(
                                        Button::new("删除")
                                            .danger()
                                            .label("删除")
                                            .on_click(cx.listener(Self::on_delete)),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_x_4()
                            .items_center()
                            .child(Text::String(
                                format!("分区: {}", room_info.area_name).into(),
                            ))
                            .child({
                                if matches!(self.status, RoomCardStatus::Recording(_)) {
                                    Text::String(
                                        format!("直播开始时间: {}", room_info.live_time).into(),
                                    )
                                } else {
                                    Text::String("".into())
                                }
                            }),
                    )
                    .children({
                        if matches!(self.status, RoomCardStatus::Recording(_)) {
                            vec![Text::String(self.status.to_string().into()).into_element()]
                        } else {
                            vec![]
                        }
                    })
                    .children({
                        if matches!(self.status, RoomCardStatus::Error(_)) {
                            vec![
                                Text::String(
                                    match &self.status {
                                        RoomCardStatus::Error(err) => err.clone(),
                                        _ => String::new(),
                                    }
                                    .into(),
                                )
                                .into_element(),
                            ]
                        } else {
                            vec![]
                        }
                    }),
            )
    }
}
