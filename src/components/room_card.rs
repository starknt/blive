use crate::{
    core::{
        HttpClient,
        downloader::{BLiveDownloader, utils::pretty_bytes},
        http_client::{
            room::{LiveRoomInfoData, LiveStatus},
            user::LiveUserInfo,
        },
    },
    logger::log_user_action,
    settings::RoomSettings,
    state::AppState,
};
use gpui::{
    App, ClickEvent, Entity, EventEmitter, ObjectFit, Subscription, WeakEntity, Window, div, img,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable, StyledExt,
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
}

#[derive(Clone, PartialEq, Debug)]
pub enum RoomCardStatus {
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
    pub(crate) status: RoomCardStatus,
    pub(crate) room_info: LiveRoomInfoData,
    pub(crate) user_info: LiveUserInfo,
    pub(crate) settings: RoomSettings,
    _subscriptions: Vec<Subscription>,
    pub downloader: Option<Arc<BLiveDownloader>>,
    pub user_stop: bool,
}

impl RoomCard {
    fn new(room: LiveRoomInfoData, user: LiveUserInfo, settings: RoomSettings) -> Self {
        Self {
            _subscriptions: vec![],
            status: match room.live_status {
                LiveStatus::Live => RoomCardStatus::Recording(0.0),
                _ => RoomCardStatus::Waiting,
            },
            room_info: room,
            user_info: user,
            settings,
            downloader: None,
            user_stop: false,
        }
    }

    pub fn view(
        room: LiveRoomInfoData,
        user: LiveUserInfo,
        settings: RoomSettings,
        cx: &mut App,
        client: HttpClient,
    ) -> Entity<Self> {
        let room_id = room.room_id;
        let live_status = room.live_status;

        let card = cx.new(|cx| {
            cx.spawn(async move |this: WeakEntity<RoomCard>, cx| {
                while let Some(this) = this.upgrade() {
                    let room_info = client.get_live_room_info(room_id).await;

                    if let Ok(room_info) = room_info {
                        let _ = this.update(cx, |this, cx| {
                            if this.room_info.live_status != room_info.live_status {
                                cx.emit(RoomCardEvent::LiveStatusChanged(room_info.live_status));
                            }
                            this.room_info = room_info.clone();
                            cx.notify();
                        });
                    }

                    cx.background_executor()
                        .timer(Duration::from_secs(15))
                        .await;
                }
            })
            .detach();

            Self::new(room, user, settings)
        });

        let subscription = cx.subscribe(&card, Self::on_event);

        card.update(cx, |card, cx| {
            card._subscriptions.push(subscription);

            if live_status == LiveStatus::Live {
                cx.emit(RoomCardEvent::StatusChanged(RoomCardStatus::Recording(0.0)));
            }
        });

        card
    }

    fn on_event(this: Entity<Self>, event: &RoomCardEvent, cx: &mut App) {
        match event {
            RoomCardEvent::LiveStatusChanged(status) => {
                this.update(cx, |card, cx| {
                    card.status = match status {
                        LiveStatus::Live => RoomCardStatus::Recording(0.0),
                        _ => RoomCardStatus::Waiting,
                    };
                    cx.emit(RoomCardEvent::StatusChanged(card.status.clone()));
                });
            }
            RoomCardEvent::StartRecording => {
                this.update(cx, |this, cx| {
                    this.user_stop = false;
                    cx.notify();
                });
            }
            RoomCardEvent::StopRecording => {
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

                        cx.update_global(|state: &mut AppState, _| {
                            state
                                .downloaders
                                .retain(|d| d.context.room_info.room_id != this.room_info.room_id);
                        });
                    }

                    this.user_stop = true;
                    cx.notify();
                });
            }
            RoomCardEvent::StatusChanged(status) => {
                match status {
                    RoomCardStatus::Recording(_speed) => {
                        if !this.read(cx).user_stop {
                            Self::do_record(this, cx);
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

                                cx.update_global(|state: &mut AppState, _| {
                                    state.downloaders.retain(|d| {
                                        d.context.room_info.room_id != this.room_info.room_id
                                    });
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
    fn do_record(this: Entity<RoomCard>, cx: &mut App) {
        let task_card = this.downgrade();
        let card = this.read(cx);
        let room_info = card.room_info.clone();
        let user_info = card.user_info.clone();
        let client = AppState::global(cx).client.clone();
        let global_setting = AppState::global(cx).settings.clone();
        let record_dir = global_setting.record_dir;

        let downloader = Arc::new(BLiveDownloader::new(
            room_info,
            user_info,
            global_setting.quality,
            global_setting.format,
            global_setting.codec,
            global_setting.strategy,
            client,
            task_card.clone(),
        ));

        this.update(cx, |this, _| {
            this.downloader = Some(downloader.clone());
        });

        AppState::global_mut(cx)
            .downloaders
            .push(downloader.clone());

        cx.spawn(async move |cx| {
            let downloader = downloader.clone();

            // 开始下载
            match downloader.start(cx, &record_dir).await {
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

impl EventEmitter<RoomCardEvent> for RoomCard {}

impl Render for RoomCard {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let room_info = &self.room_info;

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
                                                        self.user_info.uname.clone().into(),
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
                                                        let new_status = match &card.status {
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
                                                                RoomCardStatus::Recording(0.0)
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
                                                                RoomCardStatus::Waiting
                                                            }
                                                            RoomCardStatus::Error(_) => {
                                                                log_user_action(
                                                                    "重试录制",
                                                                    Some(&format!(
                                                                        "房间号: {room_id}"
                                                                    )),
                                                                );
                                                                RoomCardStatus::Waiting
                                                            }
                                                        };
                                                        card.status = new_status.clone();
                                                        cx.notify();
                                                    })),
                                            ])
                                        } else {
                                            h_flex().flex_1()
                                        }
                                    })
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
