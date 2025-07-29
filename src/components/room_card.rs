use crate::{
    core::{
        HttpClient,
        downloader::BilibiliDownloader,
        http_client::{
            room::{LiveRoomInfoData, LiveStatus},
            user::LiveUserInfo,
        },
    },
    settings::RoomSettings,
    state::AppState,
};
use gpui::{
    App, ClickEvent, Entity, EventEmitter, ObjectFit, Subscription, Task, WeakEntity, Window, div,
    img, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    text::Text,
    v_flex,
};
use std::time::Duration;

#[derive(Clone, Debug)]
enum RoomCardEvent {
    LiveStatusChanged(LiveStatus),
    StatusChanged(RoomCardStatus),
}

#[derive(Clone, PartialEq, Eq, Debug, Copy)]
pub enum RoomCardStatus {
    Waiting,
    Recording,
    Error,
}

pub struct RoomCard {
    pub(crate) _tasks: Vec<Task<()>>,
    pub(crate) status: RoomCardStatus,
    pub(crate) error_message: Option<String>,
    pub(crate) room_info: LiveRoomInfoData,
    pub(crate) user_info: LiveUserInfo,
    pub(crate) settings: RoomSettings,
    _subscriptions: Vec<Subscription>,
    downloader: Option<BilibiliDownloader>,
}

impl RoomCard {
    fn new(
        room: LiveRoomInfoData,
        user: LiveUserInfo,
        settings: RoomSettings,
        task: Task<()>,
    ) -> Self {
        Self {
            _tasks: vec![task],
            _subscriptions: vec![],
            status: match room.live_status {
                LiveStatus::Live => RoomCardStatus::Recording,
                _ => RoomCardStatus::Waiting,
            },
            error_message: None,
            room_info: room,
            user_info: user,
            settings,
            downloader: None,
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
            let task = cx.spawn(async move |this: WeakEntity<RoomCard>, cx| {
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
            });

            Self::new(room, user, settings, task)
        });

        let subscriptions = vec![cx.subscribe(&card, Self::on_event)];

        card.update(cx, |card, cx| {
            card._subscriptions = subscriptions;

            if live_status == LiveStatus::Live {
                cx.emit(RoomCardEvent::StatusChanged(RoomCardStatus::Recording));
            }
        });

        card
    }

    fn on_event(this: Entity<Self>, event: &RoomCardEvent, cx: &mut App) {
        match event {
            RoomCardEvent::LiveStatusChanged(status) => {
                this.update(cx, |card, cx| {
                    card.status = match status {
                        LiveStatus::Live => RoomCardStatus::Recording,
                        _ => RoomCardStatus::Waiting,
                    };
                    cx.emit(RoomCardEvent::StatusChanged(card.status));
                });
            }
            RoomCardEvent::StatusChanged(status) => {
                match *status {
                    RoomCardStatus::Recording => {
                        Self::do_record(this, cx);
                    }
                    RoomCardStatus::Waiting => {
                        // 停止录制
                        this.update(cx, |this, cx| {
                            this.status = RoomCardStatus::Waiting;
                            // 停止下载器
                            if let Some(ref mut downloader) = this.downloader {
                                downloader.stop();
                            }
                            cx.notify();
                        });
                    }
                    RoomCardStatus::Error => {
                        // 错误
                    }
                }
            }
        }
    }

    fn on_delete(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut gpui::Context<Self>) {
        let this = cx.entity();
        cx.update_global(|state: &mut AppState, _| {
            state.room_entities = state
                .room_entities
                .iter()
                .filter(|room| room.entity_id() != this.entity_id())
                .cloned()
                .collect();
            state.settings.rooms = state
                .settings
                .rooms
                .iter()
                .filter(|room| room.room_id != self.settings.room_id)
                .cloned()
                .collect();
        });
    }
}

impl RoomCard {
    fn do_record(this: Entity<RoomCard>, cx: &mut App) {
        let task_card = this.downgrade();
        let card = this.read(cx);
        let room_info = card.room_info.clone();
        let user_info = card.user_info.clone();
        let _room_settings = card.settings.clone();
        let client = AppState::global(cx).client.clone();
        let global_setting = AppState::global(cx).settings.clone();
        let record_dir = global_setting.record_dir;

        cx.spawn(async move |cx| {
            // 创建下载器
            let mut downloader = BilibiliDownloader::new(
                room_info.room_id,
                global_setting.quality.to_quality(),
                global_setting.format,
                global_setting.codec,
                client,
            );

            // 开始下载
            match downloader
                .start_download(cx, &room_info, &user_info, &record_dir)
                .await
            {
                Ok(_) => {
                    let _ = task_card.update(cx, |card, _| {
                        card.status = RoomCardStatus::Waiting;
                        card.error_message = None;
                    });
                }
                Err(e) => {
                    let _ = task_card.update(cx, |card, _| {
                        card.status = RoomCardStatus::Error;
                        card.error_message = Some(format!("下载失败: {e}"));
                    });
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
                RoomCardStatus::Error => gpui::rgb(0xef4444),
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
                                // 左侧：封面和基本信息
                                h_flex()
                                    .gap_3()
                                    .items_start()
                                    .child(
                                        // 封面图片
                                        div().w_32().h_20().rounded_lg().overflow_hidden().child(
                                            div().rounded_lg().overflow_hidden().size_full().child(
                                                img(room_info.user_cover.clone())
                                                    .block()
                                                    .size_full()
                                                    .object_fit(ObjectFit::Cover),
                                            ),
                                        ),
                                    )
                                    .child(
                                        // 房间信息
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
                                                format!("房间号: {}", room_info.room_id)
                                                    .into_element(),
                                            )
                                            .child(
                                                // 直播状态和在线人数
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        // 直播状态指示器
                                                        div().w_2().h_2().rounded_full().bg(
                                                            match room_info.live_status {
                                                                LiveStatus::Live => {
                                                                    gpui::rgb(0xef4444)
                                                                }
                                                                _ => gpui::rgb(0x6b7280),
                                                            },
                                                        ),
                                                    )
                                                    .child(Text::String(
                                                        match room_info.live_status {
                                                            LiveStatus::Live => "直播中".into(),
                                                            LiveStatus::Carousel => "轮播中".into(),
                                                            LiveStatus::Offline => "未开播".into(),
                                                        },
                                                    ))
                                                    .when(
                                                        self.status == RoomCardStatus::Recording,
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
                                        if self.status == RoomCardStatus::Recording {
                                            h_flex().flex_1().children(vec![
                                                Button::new("record")
                                                    .primary()
                                                    .label(match self.status {
                                                        RoomCardStatus::Waiting => {
                                                            "开始录制".into()
                                                        }
                                                        RoomCardStatus::Recording => {
                                                            "停止录制".into()
                                                        }
                                                        RoomCardStatus::Error => format!(
                                                            "错误: {}",
                                                            self.error_message
                                                                .clone()
                                                                .unwrap_or_default()
                                                        ),
                                                    })
                                                    .on_click(cx.listener(|card, _, _, cx| {
                                                        card.status = match card.status {
                                                            RoomCardStatus::Waiting => {
                                                                RoomCardStatus::Recording
                                                            }
                                                            RoomCardStatus::Recording => {
                                                                RoomCardStatus::Waiting
                                                            }
                                                            RoomCardStatus::Error => {
                                                                RoomCardStatus::Waiting
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
                                if self.status == RoomCardStatus::Recording {
                                    Text::String(
                                        format!("直播开始时间: {}", room_info.live_time).into(),
                                    )
                                } else {
                                    Text::String("".into())
                                }
                            }),
                    )
                    .children({
                        if self.status == RoomCardStatus::Error {
                            vec![
                                Text::String(self.error_message.clone().unwrap_or_default().into())
                                    .into_element(),
                            ]
                        } else {
                            vec![]
                        }
                    }),
            )
    }
}
