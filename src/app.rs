use gpui::{
    App, AppContext, Axis, ClickEvent, Entity, EventEmitter, Subscription, Window, div, prelude::*,
    px,
};
use gpui_component::{
    ActiveTheme as _, ContextModal, Root, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    notification::Notification,
    text::Text,
    v_flex,
};

use crate::{
    components::{RoomCard, RoomCardStatus, RoomInput, RoomInputEvent},
    logger::log_user_action,
    settings::RoomSettings,
    state::AppState,
    title_bar::AppTitleBar,
};

enum BLiveAppEvent {
    InitRoom(RoomSettings),
}

pub struct BLiveApp {
    room_id: u64,
    room_input: Entity<RoomInput>,
    title_bar: Entity<AppTitleBar>,
    room_cards: Vec<Entity<RoomCard>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<BLiveAppEvent> for BLiveApp {}

impl BLiveApp {
    fn new(
        title: String,
        rooms: Vec<RoomSettings>,
        reopen: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new(title, window, cx));
        let room_num = 1804892069;
        let room_input = RoomInput::view(room_num, window, cx);

        let _subscriptions = vec![
            cx.subscribe_in(&room_input, window, Self::on_room_input_change),
            cx.subscribe_in(&cx.entity(), window, Self::on_app_event),
        ];

        let mut room_cards = vec![];

        if !reopen {
            for room in rooms {
                let room_id = room.room_id;
                log_user_action("加载房间", Some(&format!("房间号: {room_id}")));
                cx.emit(BLiveAppEvent::InitRoom(room));
            }
        } else {
            let (client, global_settings, room_states) = cx.read_global(|state: &AppState, _| {
                (
                    state.client.clone(),
                    state.settings.clone(),
                    state.room_states.clone(),
                )
            });

            for room in room_states {
                room_cards.push(cx.new(|cx| {
                    RoomCard::view(
                        RoomSettings {
                            room_id: room.room_id,
                            strategy: Some(
                                room.settings.strategy.unwrap_or(global_settings.strategy),
                            ),
                            quality: Some(room.settings.quality.unwrap_or(global_settings.quality)),
                            format: Some(room.settings.format.unwrap_or(global_settings.format)),
                            codec: Some(room.settings.codec.unwrap_or(global_settings.codec)),
                            record_name: room.settings.record_name.clone(),
                            record_dir: match room
                                .settings
                                .record_dir
                                .clone()
                                .unwrap_or_default()
                                .is_empty()
                            {
                                true => Some(global_settings.record_dir.clone()),
                                false => room.settings.record_dir.clone(),
                            },
                        },
                        client.clone(),
                        room.downloader.clone(),
                        window,
                        cx,
                    )
                }));
            }
        }

        Self {
            room_id: room_num,
            room_input,
            title_bar,
            room_cards,
            _subscriptions,
        }
    }

    /// 创建应用视图
    pub fn view(
        title: String,
        rooms: Vec<RoomSettings>,
        reopen: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(title, rooms, reopen, window, cx))
    }

    /// 处理房间输入变化
    fn on_room_input_change(
        &mut self,
        _: &Entity<RoomInput>,
        event: &RoomInputEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        let RoomInputEvent::RoomInputChange(room_num) = event;
        self.room_id = *room_num;
        log_user_action("房间号输入变化", Some(&format!("新房间号: {room_num}")));
    }

    /// 添加录制房间
    fn add_recording(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.room_id > 0 {
            let room_id = self.room_id;

            log_user_action("点击添加录制按钮", Some(&format!("房间号: {room_id}")));

            cx.update_global(|state: &mut AppState, cx| {
                // 检查是否已经存在
                if state
                    .settings
                    .rooms
                    .iter()
                    .any(|room| room.room_id == room_id)
                {
                    log_user_action("尝试添加重复房间", Some(&format!("房间号: {room_id}")));
                    window.push_notification(
                        Notification::warning(format!("不能重复监听 {room_id}")),
                        cx,
                    );
                    return;
                }

                let global_settings = state.settings.clone();
                let settings = RoomSettings::new(room_id);
                state.add_room_state(room_id, settings.clone());
                state.settings.rooms.push(settings.clone());
                self.room_cards.push(cx.new(|cx| {
                    RoomCard::view(
                        RoomSettings {
                            room_id,
                            strategy: Some(settings.strategy.unwrap_or(global_settings.strategy)),
                            quality: Some(settings.quality.unwrap_or(global_settings.quality)),
                            format: Some(settings.format.unwrap_or(global_settings.format)),
                            codec: Some(settings.codec.unwrap_or(global_settings.codec)),
                            record_name: settings.record_name.clone(),
                            record_dir: match settings
                                .record_dir
                                .clone()
                                .unwrap_or_default()
                                .is_empty()
                            {
                                true => Some(global_settings.record_dir.clone()),
                                false => settings.record_dir.clone(),
                            },
                        },
                        state.client.clone(),
                        None,
                        window,
                        cx,
                    )
                }));
                cx.notify();
                log_user_action("新房间添加成功", Some(&format!("房间号: {room_id}")));
            });
        } else {
            log_user_action("尝试添加无效房间", Some("房间号为0"));
        }
    }
}

impl BLiveApp {
    fn on_app_event(
        &mut self,
        _: &Entity<Self>,
        event: &BLiveAppEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            BLiveAppEvent::InitRoom(settings) => {
                cx.update_global(|state: &mut AppState, cx| {
                    state.add_room_state(settings.room_id, settings.clone());
                    let downloader = state
                        .get_room_state(settings.room_id)
                        .and_then(|room| room.downloader.clone());
                    let room_card = cx.new(|cx| {
                        RoomCard::view(
                            settings.clone(),
                            state.client.clone(),
                            downloader,
                            window,
                            cx,
                        )
                    });
                    self.room_cards.push(room_card);
                    log_user_action(
                        "房间状态创建成功",
                        Some(&format!("房间号: {}", settings.room_id)),
                    );
                });
            }
        }

        cx.notify();
    }
}

impl Render for BLiveApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modal_layer = Root::render_modal_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let state = AppState::global(cx);
        let recording_count = state
            .room_states
            .iter()
            .filter(|room| matches!(room.status, RoomCardStatus::LiveRecording))
            .count();

        div()
            .size_full()
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .min_w_full()
            .min_h_full()
            .child(self.title_bar.clone())
            .child(
                v_flex()
                .flex_1()
                    .scrollable(Axis::Vertical)
                    .size_full()
                    .pb_6()
                    .child(
                        div()
                            .flex_1()
                            .p_8()
                            .overflow_hidden()
                            .child(
                                v_flex()
                                    .size_full()
                                    .gap_8()
                                    .child(
                                        div()
                                            .rounded_xl()
                                            .p_8()
                                            .bg(cx.theme().primary)
                                            .border_color(cx.theme().border)
                                            .child(
                                                v_flex()
                                                    .gap_4()
                                                    .child(
                                                        div()
                                                            .font_bold()
                                                            .text_2xl()
                                                            .text_color(cx.theme().primary_foreground)
                                                            .child(Text::String("B站直播录制器".into())),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(cx.theme().accent)
                                                            .child(Text::String("轻松录制B站直播，支持多房间同时录制".into())),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .rounded_xl()
                                            .p_6()
                                            .border(px(1.0))
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().background)
                                            .shadow_lg()
                                            .child(
                                                v_flex()
                                                    .gap_6()
                                                    .child(
                                                        div()
                                                            .font_bold()
                                                            .text_lg()
                                                            .child(Text::String("添加录制房间".into())),
                                                    )
                                                    .child(
                                                        div()
                                                            .rounded_lg()
                                                            .p_4()
                                                            .bg(cx.theme().background)
                                                            .child(
                                                                v_flex()
                                                                    .gap_4()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .text_color(cx.theme().accent_foreground)
                                                                            .child(Text::String("请输入B站直播间房间号".into())),
                                                                    )
                                                                    .child(
                                                                        h_flex()
                                                                            .max_w_96()
                                                                            .gap_4()
                                                                            .items_center()
                                                                            .child(
                                                                                div()
                                                                                    .flex_1()
                                                                                    .child(self.room_input.clone()),
                                                                            )
                                                                            .child(
                                                                                Button::new("添加录制")
                                                                                    .on_click(cx.listener(Self::add_recording))
                                                                                    .primary()
                                                                                    .child(Text::String("添加录制".into())),
                                                                            ),
                                                                    ),
                                                            ),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        // 房间列表卡片
                                        div()
                                            .flex_1()
                                            .rounded_xl()
                                            .p_6()
                                            .border(px(1.0))
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().background)
                                            .shadow_lg()
                                            .child(
                                                v_flex()
                                                    .size_full()
                                                    .gap_6()
                                                    .child(
                                                        // 标题栏
                                                        h_flex()
                                                            .justify_between()
                                                            .items_center()
                                                            .child(
                                                                div()
                                                                    .font_bold()
                                                                    .text_lg()
                                                                    .child(Text::String("录制房间列表".into())),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px_3()
                                                                    .py_1()
                                                                    .rounded_full()
                                                                    .bg(cx.theme().card)
                                                                    .text_sm()
                                                                    .font_semibold()
                                                                    .text_color(cx.theme().primary)
                                                                                                        .child(Text::String(
                                        format!("共 {} 个房间", state.room_states.len()).into(),
                                    )),
                                                            ),
                                                    )
                                                    .child(
                                                        // 统计信息
                                                        div()
                                                            .rounded_lg()
                                                            .p_4()
                                                            .border_color(cx.theme().border)
                                                            .child(
                                                                v_flex()
                                                                    .gap_3()
                                                                    .child(
                                                                        div()
                                                                            .font_semibold()
                                                                            .text_lg()
                                                                            .child(Text::String("录制统计".into())),
                                                                    )
                                                                    .child(
                                                                        h_flex()
                                                                            .gap_6()
                                                                            .child(
                                                                                div()
                                                                                    .text_center()
                                                                                    .child(
                                                                                        v_flex()
                                                                                            .gap_1()
                                                                                            .child(
                                                                                                div()
                                                                                                    .font_semibold()
                                                                                                    .text_2xl()
                                                                                                    .text_color(gpui::rgb(0x3b82f6))
                                                                                                    .child(Text::String(state.room_states.len().to_string().into())),
                                                                                            )
                                                                                            .child(
                                                                                                div()
                                                                                                    .text_sm()
                                                                                                    .text_color(cx.theme().accent_foreground)
                                                                                                    .child(Text::String("总房间数".into())),
                                                                                            ),
                                                                                    ),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_center()
                                                                                    .child(
                                                                                        v_flex()
                                                                                            .gap_1()
                                                                                            .child(
                                                                                                div()
                                                                                                    .font_semibold()
                                                                                                    .text_2xl()
                                                                                                    .text_color(gpui::rgb(0x10b981))
                                                                                                    .child(Text::String(recording_count.to_string().into())),
                                                                                            )
                                                                                            .child(
                                                                                                div()
                                                                                                    .text_sm()
                                                                                                    .text_color(cx.theme().accent_foreground)
                                                                                                    .child(Text::String("录制中".into())),
                                                                                            ),
                                                                                    ),
                                                                            )
                                                                    ),
                                                            ),
                                                    )
                                                    .child({
                                                        if !state.room_states.is_empty() {
                                                            div()
                                                                .flex_1()
                                                                .overflow_hidden()
                                                                .child(
                                                                    v_flex()
                                                                        .size_full()
                                                                        .gap_4()
                                                                        .scrollable(Axis::Vertical)
                                                                        .children(self.room_cards.to_vec()),
                                                                )
                                                        } else {
                                                            div()
                                                                .flex_1()
                                                                .flex()
                                                                .justify_center()
                                                                .items_center()
                                                                .child(
                                                                    div()
                                                                        .text_center()
                                                                        .child(
                                                                            v_flex()
                                                                                .gap_4()
                                                                                .items_center()
                                                                                .child(
                                                                                    div()
                                                                                        .w_16()
                                                                                        .h_16()
                                                                                        .rounded_full()
                                                                                        .bg(cx.theme().accent)
                                                                                        .flex()
                                                                                        .justify_center()
                                                                                        .items_center()
                                                                                        .child(
                                                                                            div()
                                                                                                .text_2xl()
                                                                                                .text_color(cx.theme().accent_foreground)
                                                                                                .child(Text::String("📺".into())),
                                                                                        ),
                                                                                )
                                                                                .child(
                                                                                    div()
                                                                                        .font_semibold()
                                                                                        .text_color(cx.theme().accent_foreground)
                                                                                        .child(Text::String("暂无录制房间".into())),
                                                                                )
                                                                                .child(
                                                                                    div()
                                                                                        .text_sm()
                                                                                        .text_color(cx.theme().accent_foreground)
                                                                                        .child(Text::String("添加房间开始录制直播".into())),
                                                                                ),
                                                                        ),
                                                                )
                                                        }
                                                    }),
                                            ),
                                    ),
                            )
                            .child(div().absolute().top_8().children(notification_layer))
                            .children(modal_layer),
                    ),
            )
    }
}
