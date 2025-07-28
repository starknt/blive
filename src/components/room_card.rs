use crate::{
    api::{
        ApiClient,
        room::{LiveRoomInfoData, LiveStatus},
        user::LiveUserInfo,
    },
    settings::{DEFAULT_RECORD_NAME, RoomSettings, StreamCodec},
    state::AppState,
};
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
use futures_util::AsyncReadExt;
use gpui::{
    App, ClickEvent, Entity, EventEmitter, ObjectFit, Subscription, Task, WeakEntity, Window, div,
    http_client::{AsyncBody, Method, Request},
    img,
    prelude::*,
    px,
};
use gpui_component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants},
    h_flex,
    text::{Text, TextView},
    v_flex,
};
use leon::Values;
use rand::Rng;
use std::{
    borrow::Cow,
    io::{ErrorKind, Write},
};
use std::{fs::File, time::Duration};

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

struct RoomCardValues {
    pub up_name: String,
    pub room_id: u64,
    pub room_title: String,
    pub room_description: String,
    pub room_area_name: String,
    pub date: String,
    pub datetime: String,
}

impl Values for RoomCardValues {
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

pub struct RoomCard {
    pub(crate) _tasks: Vec<Task<()>>,
    pub(crate) status: RoomCardStatus,
    pub(crate) error_message: Option<String>,
    pub(crate) room_info: LiveRoomInfoData,
    pub(crate) user_info: LiveUserInfo,
    pub(crate) settings: RoomSettings,
    _subscriptions: Vec<Subscription>,
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
        }
    }

    pub fn view(
        room: LiveRoomInfoData,
        user: LiveUserInfo,
        settings: RoomSettings,
        cx: &mut App,
        client: ApiClient,
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

    fn on_delete(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
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
        let room_settings = card.settings.clone();
        let client = AppState::global(cx).client.clone();
        let http_client = cx.http_client().clone();
        let global_setting = AppState::global(cx).settings.clone();
        let record_dir = global_setting.record_dir;

        cx.spawn(async move |cx| {
            if let Ok(data) = client.get_live_room_stream_url(room_info.room_id, global_setting.quality.to_quality()).await
                && let Some(info) = data.playurl_info
            {
                let stream = info
                    .playurl
                    .stream
                    .iter()
                    .find(|stream| stream.protocol_name == "http_stream");

                if stream.is_none() || stream.unwrap().format.is_empty() {
                    let _ = task_card.update(cx, |card, _| {
                        card.status = RoomCardStatus::Error;
                        card.error_message = Some("未找到合适的直播流".to_string());
                    });

                    return Err(anyhow::anyhow!("Failed to get stream"));
                }

                // 1. 优先选择配置中的格式
                let format_stream = {
                    let format_stream = stream.unwrap()
                        .format
                        .iter()
                        .find(|format| format.format_name.as_str() == global_setting.format.as_str());

                    if format_stream.is_none() {
                        let _ = task_card.update(cx, |card, _| {
                            card.status = RoomCardStatus::Error;
                            card.error_message = Some("未找到合适的直播流".to_string());
                        });

                        return Err(anyhow::anyhow!("Failed to get stream"));
                    }

                    format_stream.unwrap_or_else(|| stream.unwrap().format.first().unwrap())
                };

                if format_stream.codec.is_empty() {
                    let _ = task_card.update(cx, |card, _| {
                        card.status = RoomCardStatus::Error;
                        card.error_message = Some("未找到合适的直播流".to_string());
                    });

                    return Err(anyhow::anyhow!("Failed to get stream"));
                }

                // 2. 优先按照设置选择编码格式 avc 或者 hevc
                let codec = format_stream.codec.iter()
                    .find(|codec| codec.codec_name == global_setting.codec)
                    .unwrap_or_else(|| format_stream.codec.first().unwrap());

                // 随机选择 url
                let info = &codec.url_info[rand::rng().random_range(0..codec.url_info.len())];
                let url = format!("{}{}{}", info.host, codec.base_url, info.extra);

                let template = leon::Template::parse(&room_settings.record_name)
                    .unwrap_or(leon::Template::parse(DEFAULT_RECORD_NAME).unwrap());

                let live_time = NaiveDateTime::parse_from_str(&room_info.live_time, "%Y-%m-%d %H:%M:%S").unwrap_or_default();
                let live_time = live_time.and_local_timezone(Shanghai).unwrap();
                let values = RoomCardValues {
                    up_name: user_info.uname.clone(),
                    room_id: room_info.room_id,
                    datetime: live_time.format("%Y-%m-%d %H点%M分").to_string(),
                    room_title: room_info.title.clone(),
                    room_description: room_info.description.clone(),
                    room_area_name: room_info.area_name.clone(),
                    date: live_time.format("%Y-%m-%d").to_string(),
                };
                let ext = match format_stream.format_name.as_str() {
                    "flv" => "flv",
                    "ts" => "m4s",
                    "fmp4" => "m4s",
                    _ => "mp4",
                };
                let file_name = template.render(&values).unwrap_or_default();
                let file_path = format!("{record_dir}/{file_name}.{ext}");

                // ensure the directory exists
                std::fs::create_dir_all(&record_dir).unwrap_or_default();

                if let Ok(mut file) = File::create(file_path) {
                    let request = Request::builder()
                        .uri(url)
                        .header("Referer", "https://live.bilibili.com/")
                        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                        .method(Method::GET)
                        .body(AsyncBody::empty())?;
                    let mut response = http_client.send(request).await?;

                    if !response.status().is_success() {
                        let _ = task_card.update(cx, |card, _| {
                            card.status = RoomCardStatus::Error;
                            card.error_message = Some("下载直播流时发生错误".to_string());
                        });

                        return Err(anyhow::anyhow!("Failed to download file"));
                    }

                    let mut buffer = match global_setting.codec {
                        StreamCodec::AVC => vec![0u8; 8192],
                        StreamCodec::HEVC => vec![0u8; 16384],
                    };

                    let mut stop = false;
                    let body = response.body_mut();

                    loop {
                        if let Ok(bytes_read) = body.read(&mut buffer).await {
                            if bytes_read == 0 {
                                return Ok(());
                            }

                            let write_result = file.write_all(&buffer[..bytes_read]);

                            if let Err(e) = write_result {
                                // 写入失败，可能是磁盘满了
                                match e.kind() {
                                  ErrorKind::StorageFull => {
                                        let _ = task_card.update(cx, |card, _| {
                                            card.status = RoomCardStatus::Error;
                                            card.error_message = Some("磁盘空间不足".to_string());
                                        });

                                        return Err(anyhow::anyhow!("Disk space is full"));
                                    },
                                  ErrorKind::Interrupted => {
                                    // 中断，继续写入
                                    continue;
                                  },
                                  ErrorKind::WouldBlock => {
                                    // 阻塞，等待写入
                                    cx.background_executor().timer(Duration::from_millis(100)).await;
                                  },
                                  ErrorKind::BrokenPipe => {
                                    // 管道破裂，停止写入
                                    break;
                                  },
                                  _ => {
                                    let _ = task_card.update(cx, |card, _| {
                                        card.status = RoomCardStatus::Error;
                                        card.error_message = Some("写入视频文件失败".to_string());
                                    });

                                    return Err(anyhow::anyhow!("Failed to write file"));
                                  }
                                }
                            }

                            // check record status
                            let _ = task_card.update(cx, |card, _| {
                                if card.status != RoomCardStatus::Recording {
                                    stop = true;
                                }
                            });

                            if stop {
                                break;
                            }
                        } else {
                            let _ = task_card.update(cx, |card, _| {
                                card.status = RoomCardStatus::Error;
                                card.error_message = Some("读取直播流时发生错误".to_string());
                            });

                            return Err(anyhow::anyhow!("Failed to read stream"));
                        }
                    }
                } else {
                    let _ = task_card.update(cx, |card, _| {
                        card.status = RoomCardStatus::Error;
                        card.error_message = Some("创建视频文件失败".to_string());
                    });

                    return Err(anyhow::anyhow!("Failed to create file"));
                }
            }

            Ok(())
        }).detach();
    }
}

impl EventEmitter<RoomCardEvent> for RoomCard {}

impl Render for RoomCard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                            .child(room_info.title.clone().into_element())
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
                                v_flex().gap_2().child(
                                    Button::new("删除")
                                        .danger()
                                        .label("删除")
                                        .on_click(cx.listener(Self::on_delete)),
                                ),
                            ),
                    )
                    // render html description
                    .child(TextView::html("description", room_info.description.clone()))
                    .child(
                        h_flex()
                            .gap_x_4()
                            .items_center()
                            .child(Text::String(
                                format!("分区: {}", room_info.area_name).into(),
                            ))
                            .child({
                                if self.status == RoomCardStatus::Recording {
                                    h_flex().flex_1().children(vec![
                                        Button::new("record")
                                            .primary()
                                            .label(match self.status {
                                                RoomCardStatus::Waiting => "开始录制".into(),
                                                RoomCardStatus::Recording => "停止录制".into(),
                                                RoomCardStatus::Error => format!(
                                                    "错误: {}",
                                                    self.error_message.clone().unwrap_or_default()
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
                            }),
                    ),
            )
    }
}

impl Drop for RoomCard {
    fn drop(&mut self) {
        self.status = RoomCardStatus::Waiting;

        for task in self._tasks.drain(..) {
            task.detach();
        }
    }
}
