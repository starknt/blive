use gpui::{App, Axis, ClickEvent, Entity, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _, ContextModal, Root, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    notification::Notification,
    text::Text,
    v_flex,
};

use crate::{
    components::{RoomCard, RoomInput, RoomInputEvent},
    logger::{log_recording_error, log_user_action},
    settings::RoomSettings,
    state::AppState,
    title_bar::AppTitleBar,
};

pub struct BLiveApp {
    room_num: u64,
    room_input: Entity<RoomInput>,
    title_bar: Entity<AppTitleBar>,
    _subscriptions: Vec<Subscription>,
}

impl BLiveApp {
    /// 初始化应用
    pub fn init(cx: &mut App) {
        log_user_action("初始化主应用组件", None);

        let state = AppState::global(cx);
        for settings in state.settings.rooms.clone() {
            let client = state.client.clone();
            let room_id = settings.room_id;

            log_user_action("加载房间", Some(&format!("房间号: {room_id}")));

            cx.spawn(async move |cx| {
                let (room_data, user_data) = futures::join!(
                    client.get_live_room_info(room_id),
                    client.get_live_room_user_info(room_id)
                );

                if let Ok(room_data) = room_data
                    && let Ok(user_data) = user_data
                {
                    cx.update_global(|state: &mut AppState, cx| {
                        let room =
                            RoomCard::view(room_data, user_data.info, settings.clone(), cx, client);

                        state.room_entities.push(room);
                        log_user_action("房间卡片创建成功", Some(&format!("房间号: {room_id}")));
                    })
                    .unwrap();
                } else {
                    log_recording_error(room_id, "获取房间信息失败");
                };
            })
            .detach();
        }
    }

    /// 创建新的应用实例
    fn new(title: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new(title, window, cx));
        let room_num = 1804892069;
        let room_input = RoomInput::view(room_num, window, cx);

        let _subscriptions = vec![cx.subscribe_in(&room_input, window, Self::on_room_input_change)];

        Self {
            room_num,
            room_input,
            title_bar,
            _subscriptions,
        }
    }

    /// 创建应用视图
    pub fn view(title: String, window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(title, window, cx))
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
        self.room_num = *room_num;
        log_user_action("房间号输入变化", Some(&format!("新房间号: {room_num}")));
    }

    /// 添加录制房间
    fn add_recording(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.room_num > 0 {
            let client = AppState::global(cx).client.clone();
            let room_num = self.room_num;

            // 检查是否已经存在
            if AppState::global(cx)
                .settings
                .rooms
                .iter()
                .any(|room| room.room_id == room_num)
            {
                log_user_action("尝试添加重复房间", Some(&format!("房间号: {room_num}")));
                window.push_notification(
                    Notification::warning(format!("直播间 {room_num} 已监听")),
                    cx,
                );
                return;
            }

            log_user_action("点击添加录制按钮", Some(&format!("房间号: {room_num}")));

            cx.spawn(async move |_, cx| {
                let (room_data, user_data) = futures::join!(
                    client.get_live_room_info(room_num),
                    client.get_live_room_user_info(room_num)
                );

                if let Ok(room_data) = room_data
                    && let Ok(user_data) = user_data
                {
                    cx.update_global(|state: &mut AppState, cx| {
                        let settings = RoomSettings::new(room_num);
                        let room =
                            RoomCard::view(room_data, user_data.info, settings.clone(), cx, client);

                        state.room_entities.push(room);
                        state.settings.rooms.push(settings);
                        log_user_action("新房间添加成功", Some(&format!("房间号: {room_num}")));
                    })
                    .unwrap();
                } else {
                    log_recording_error(room_num, "获取房间信息失败");
                };
            })
            .detach();
        } else {
            log_user_action("尝试添加无效房间", Some("房间号为0"));
        }
    }
}

impl Render for BLiveApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modal_layer = Root::render_modal_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let state = AppState::global(cx);

        div()
            .size_full()
            .bg(cx.theme().background)
            .child(self.title_bar.clone())
            .child(
                v_flex()
                    .scrollable(Axis::Vertical)
                    .size_full()
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
                                        // 欢迎区域
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
                                                            .text_color(cx.theme().accent_foreground)
                                                            .child(Text::String("轻松录制B站直播，支持多房间同时录制".into())),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        // 输入区域卡片
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
                                                                        format!("共 {} 个房间", state.room_entities.len()).into(),
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
                                                                                                    .child(Text::String(state.room_entities.len().to_string().into())),
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
                                                                                                    .child(Text::String("0".into())),
                                                                                            )
                                                                                            .child(
                                                                                                div()
                                                                                                    .text_sm()
                                                                                                    .text_color(cx.theme().accent_foreground)
                                                                                                    .child(Text::String("录制中".into())),
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
                                                                                                    .text_color(gpui::rgb(0xef4444))
                                                                                                    .child(Text::String("0".into())),
                                                                                            )
                                                                                            .child(
                                                                                                div()
                                                                                                    .text_sm()
                                                                                                    .text_color(cx.theme().accent_foreground)
                                                                                                    .child(Text::String("错误".into())),
                                                                                            ),
                                                                                    ),
                                                                            ),
                                                                    ),
                                                            ),
                                                    )
                                                    .child({
                                                        if !state.room_entities.is_empty() {
                                                            div()
                                                                .flex_1()
                                                                .overflow_hidden()
                                                                .child(
                                                                    v_flex()
                                                                        .size_full()
                                                                        .gap_4()
                                                                        .scrollable(Axis::Vertical)
                                                                        .children(state.room_entities.to_vec()),
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
                                                                                        .bg(gpui::rgb(0x6b7280))
                                                                                        .flex()
                                                                                        .justify_center()
                                                                                        .items_center()
                                                                                        .child(
                                                                                            div()
                                                                                                .text_2xl()
                                                                                                .text_color(gpui::rgb(0x6b7280))
                                                                                                .child(Text::String("📺".into())),
                                                                                        ),
                                                                                )
                                                                                .child(
                                                                                    div()
                                                                                        .font_semibold()
                                                                                        .text_color(gpui::rgb(0x374151))
                                                                                        .child(Text::String("暂无录制房间".into())),
                                                                                )
                                                                                .child(
                                                                                    div()
                                                                                        .text_sm()
                                                                                        .text_color(gpui::rgb(0x9ca3af))
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
