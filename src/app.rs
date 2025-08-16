use std::{sync::Arc, time::Duration};

use gpui::{
    App, AppContext, Axis, Entity, EventEmitter, Subscription, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, ContextModal, Root, StyledExt, h_flex, notification::Notification,
    text::Text, v_flex,
};

use crate::{
    components::{RoomCard, RoomCardEvent, RoomCardStatus, RoomInput, RoomInputEvent},
    core::{downloader::BLiveDownloader, http_client::room::LiveStatus},
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new(title, window, cx));
        let room_id = 1804892069;
        let room_input = RoomInput::view(room_id, window, cx);

        let _subscriptions = vec![
            cx.subscribe_in(&room_input, window, Self::on_room_input_change),
            cx.subscribe_in(&cx.entity(), window, Self::on_app_event),
        ];

        for room in rooms {
            let room_id = room.room_id;
            log_user_action("加载房间", Some(&format!("房间号: {room_id}")));
            cx.emit(BLiveAppEvent::InitRoom(room));
        }

        Self {
            room_id,
            room_input,
            title_bar,
            room_cards: vec![],
            _subscriptions,
        }
    }

    pub fn view(
        title: String,
        rooms: Vec<RoomSettings>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(title, rooms, window, cx))
    }

    /// 处理房间输入变化
    fn on_room_input_change(
        &mut self,
        _: &Entity<RoomInput>,
        event: &RoomInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let RoomInputEvent::RoomInputSubmit(room_id) = event;
        self.room_id = *room_id;

        let room_id = self.room_id;

        log_user_action("点击添加录制按钮", Some(&format!("房间号: {room_id}")));

        cx.update_global(|state: &mut AppState, cx| {
            // 检查是否已经存在
            if state.has_room(room_id) {
                log_user_action("尝试添加重复房间", Some(&format!("房间号: {room_id}")));
                window.push_notification(
                    Notification::warning(format!("不能重复监听 {room_id}")),
                    cx,
                );
            } else {
                let settings = RoomSettings::new(room_id);
                state.add_room(settings.clone());
                cx.emit(BLiveAppEvent::InitRoom(settings));
                log_user_action("新房间添加成功", Some(&format!("房间号: {room_id}")));
            }
        });
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
                    let room_id = settings.room_id;

                    if !state.has_room_state(room_id) {
                        state.add_room_state(room_id);

                        let client = state.client.clone();
                        cx.spawn(async move |_, cx| {
                            loop {
                                let (room_data, user_data) = futures::join!(
                                    client.get_live_room_info(room_id),
                                    client.get_live_room_user_info(room_id)
                                );

                                match (room_data, user_data) {
                                            (Ok(room_info), Ok(user_info)) => {
                                                let _ = cx.update_global(|state: &mut AppState, cx| {
                                                    let global_settings = state.settings.clone();
                                                    let room_settings = state.get_room_settings(room_id).cloned();

                                                    if let (Some(room_state), Some(mut room_settings)) =
                                                        (state.get_room_state_mut(room_id), room_settings)
                                                    {
                                                        let room_settings = room_settings.merge_global(&global_settings);

                                                        let live_status = room_info.live_status;
                                                        room_state.room_info = Some(room_info);
                                                        room_state.user_info = Some(user_info.info);

                                                        match live_status {
                                                            LiveStatus::Live => {
                                                                if room_state.user_stop {
                                                                    return;
                                                                }

                                                                if room_state.downloader.is_some()
                                                                    && room_state
                                                                        .downloader
                                                                        .as_ref()
                                                                        .unwrap()
                                                                        .is_running()
                                                                {
                                                                    return;
                                                                }

                                                                let record_dir = room_settings.record_dir.clone().unwrap_or_default();
                                                                match room_state.downloader.clone() {
                                                                    Some(downloader) => {
                                                                        cx.spawn(async move |cx| {
                                                                            match downloader
                                                                                .start(cx, &record_dir)
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
                                                                        }).detach();
                                                                    }
                                                                    None => {
                                                                        let room_info = room_state.room_info.clone().unwrap_or_default();
                                                                        let user_info = room_state.user_info.clone().unwrap_or_default();
                                                                        let client = client.clone();
                                                                        let setting = room_settings.clone();

                                                                        let downloader = Arc::new(BLiveDownloader::new(
                                                                            room_info,
                                                                            user_info,
                                                                            setting.quality.unwrap_or_default(),
                                                                            setting.format.unwrap_or_default(),
                                                                            setting.codec.unwrap_or_default(),
                                                                            setting.strategy.unwrap_or_default(),
                                                                            client,
                                                                            room_id,
                                                                        ));

                                                                        room_state.downloader = Some(downloader.clone());

                                                                        cx.spawn(async move |cx| {
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
                                                            }
                                                            LiveStatus::Offline | LiveStatus::Carousel => {
                                                                if room_state.downloader.is_some() {
                                                                    if let Some(downloader) =
                                                                        room_state.downloader.take()
                                                                    {
                                                                        cx.foreground_executor()
                                                                            .spawn(async move {
                                                                                downloader.stop().await;
                                                                            })
                                                                            .detach();

                                                                        room_state.downloader = None;
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        if let Some(entity) = room_state.entity.clone() {
                                                            cx.notify(entity.entity_id());
                                                        }
                                                    }
                                                });
                                            }
                                            (Ok(room_info), Err(_)) => {
                                                let _ = cx.update_global(|state: &mut AppState, cx| {
                                                    if let Some(room_state) =
                                                        state.get_room_state_mut(room_id)
                                                    {
                                                        room_state.room_info = Some(room_info);

                                                        if let Some(entity) = room_state.entity.clone() {
                                                            cx.notify(entity.entity_id());
                                                        }
                                                    }
                                                });
                                            }
                                            (Err(_), Ok(user_info)) => {
                                                let _ = cx.update_global(|state: &mut AppState, cx| {
                                                    if let Some(room_state) =
                                                        state.get_room_state_mut(room_id)
                                                    {
                                                        room_state.user_info = Some(user_info.info);

                                                        if let Some(entity) = room_state.entity.clone() {
                                                            cx.notify(entity.entity_id());
                                                        }
                                                    }
                                                });
                                            }
                                            (Err(_), Err(_)) => {
                                                // nothing
                                            }
                                }

                                cx.background_executor()
                                    .timer(Duration::from_secs(10))
                                    .await;

                                // 检查房间是否移除
                                if let Some(removed) = cx.try_read_global(|state: &AppState, _| !state.has_room(room_id)) {
                                    if removed {
                                        break;
                                    }
                                }

                                let _ = cx.update_global(|state: &mut AppState, cx| {
                                    let global_settings = state.settings.clone();
                                    let room_settings = state.get_room_settings(room_id).cloned();

                                    if let (Some(room_state), Some(mut room_settings)) =
                                        (state.get_room_state_mut(room_id), room_settings)
                                    {
                                        let room_settings = room_settings.merge_global(&global_settings);
                                        if room_state.reconnecting {
                                            if room_state.reconnect_manager.should_reconnect() {
                                                let delay = room_state.reconnect_manager.calculate_delay();
                                                let record_dir = room_settings.record_dir.clone().unwrap_or_default();

                                                if let Some(downloader) = room_state.downloader.clone() {
                                                    cx.spawn(async move |cx| {
                                                        cx.background_executor().timer(delay).await;
                                                        let _ = downloader.restart(cx, &record_dir).await;
                                                    })
                                                    .detach();
                                                }

                                                room_state.reconnect_manager.increment_attempt();
                                                room_state.reconnecting = false;
                                            }
                                        }
                                    }
                                });
                            }
                        })
                        .detach();
                    }

                    let room_state = state.get_room_state_mut(room_id);
                    let downloader = room_state.as_ref().and_then(|s| s.downloader.clone());

                    let room_card = cx
                        .new(|cx| RoomCard::view(settings.clone(),  downloader, window, cx));

                    let subscription = cx.subscribe(&room_card, Self::on_room_card_event);
                    self._subscriptions.push(subscription);
                    self.room_cards.push(room_card.clone());

                    if let Some(room_state) = room_state {
                        room_state.entity = Some(room_card.downgrade());
                    }

                    log_user_action(
                        "房间创建成功",
                        Some(&format!("房间号: {}", room_id)),
                    );
                });
            }
        }
    }

    fn on_room_card_event(
        &mut self,
        _: Entity<RoomCard>,
        event: &RoomCardEvent,
        _: &mut Context<Self>,
    ) {
        if let RoomCardEvent::Deleted(entity_id) = event {
            self.room_cards
                .retain(|card| card.entity_id() != *entity_id);
        }
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
                                    .child(self.room_input.clone())
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
