use gpui::{App, Axis, ClickEvent, Entity, Subscription, Window, div, prelude::*};
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

        div().size_full().child(
            v_flex().size_full().child(self.title_bar.clone()).child(
                div()
                    .flex_1()
                    .p_6()
                    .overflow_hidden()
                    .child(
                        v_flex()
                            .justify_start()
                            .gap_6()
                            .child(
                                // 输入区域卡片
                                div()
                                    .rounded_lg()
                                    .p_4()
                                    .border(gpui::px(1.0))
                                    .border_color(cx.theme().border)
                                    .child(
                                        v_flex()
                                            .gap_4()
                                            .child(Text::String("添加录制房间".into()))
                                            .child(
                                                h_flex()
                                                    .max_w_128()
                                                    .gap_3()
                                                    .items_center()
                                                    .child(self.room_input.clone())
                                                    .child(
                                                        Button::new("添加录制")
                                                            .on_click(
                                                                cx.listener(Self::add_recording),
                                                            )
                                                            .primary()
                                                            .child(Text::String("添加录制".into())),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                // 房间列表卡片
                                div()
                                    .flex_1()
                                    .rounded_lg()
                                    .p_4()
                                    .border(gpui::px(1.0))
                                    .border_color(cx.theme().border)
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .scrollable(Axis::Vertical)
                                            .gap_4()
                                            .child(
                                                h_flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(Text::String("录制房间列表".into()))
                                                    .child(Text::String(
                                                        format!(
                                                            "共 {} 个房间",
                                                            state.room_entities.len()
                                                        )
                                                        .into(),
                                                    )),
                                            )
                                            .child({
                                                if !state.room_entities.is_empty() {
                                                    v_flex()
                                                        .gap_y_3()
                                                        .children(state.room_entities.to_vec())
                                                } else {
                                                    div()
                                                        .p_8()
                                                        .justify_center()
                                                        .items_center()
                                                        .child(Text::String("暂无录制房间".into()))
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
