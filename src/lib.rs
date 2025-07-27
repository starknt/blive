pub mod api;
pub mod assets;
pub mod components;
pub mod settings;
pub mod state;
pub mod themes;
pub mod title_bar;

use std::sync::Arc;

use futures_util::join;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Disableable, Root,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState, NumberInputEvent, StepAction, TextInput},
    text::Text,
    v_flex,
};

use crate::{components::RoomCard, state::AppState, title_bar::AppTitleBar};

pub struct LiveRecoderApp {
    room_num: u64,
    room_input: Entity<InputState>,
    title_bar: Entity<AppTitleBar>,
    _subscriptions: Vec<Subscription>,
    lock: bool,
}

impl LiveRecoderApp {
    fn on_room_input_change(
        &mut self,
        this: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.lock {
            self.lock = false;
            return;
        }

        if let InputEvent::Change(text) = event {
            if let Ok(value) = text.parse::<u64>() {
                self.room_num = value;
            }

            this.update(cx, |input, cx| {
                self.lock = true;
                input.set_value(self.room_num.to_string(), window, cx);
            });
        }
    }

    fn on_room_input_event(
        &mut self,
        this: &Entity<InputState>,
        event: &NumberInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            NumberInputEvent::Step(step_action) => match step_action {
                StepAction::Decrement => {
                    if self.room_num > 0 {
                        self.room_num = self.room_num.saturating_sub(1);
                    }

                    this.update(cx, |input, cx| {
                        input.set_value(self.room_num.to_string(), window, cx);
                    });
                }
                StepAction::Increment => {
                    self.room_num = self.room_num.saturating_add(1);

                    this.update(cx, |input, cx| {
                        input.set_value(self.room_num.to_string(), window, cx);
                    });
                }
            },
        }
    }
}

impl LiveRecoderApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new("Live Recorder".into(), window, cx));
        let room_num = 1804892069;
        let room_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("请输入直播间房间号")
                .default_value(room_num.to_string())
        });

        let _subscriptions = vec![
            cx.subscribe_in(&room_input, window, Self::on_room_input_change),
            cx.subscribe_in(&room_input, window, Self::on_room_input_event),
        ];

        Self {
            room_num,
            room_input,
            title_bar,
            _subscriptions,
            lock: false,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn add_recording(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.room_num > 0 {
            let client = Arc::clone(&AppState::global(cx).client);
            let room_num = self.room_num;

            // 检查是否已经存在
            if AppState::global(cx)
                .settings
                .rooms
                .iter()
                .any(|room| room.room_id == room_num)
            {
                return;
            }

            cx.spawn(async move |_, cx| {
                let (room_data, user_data) = join!(
                    client.get_live_room_info(room_num),
                    client.get_live_room_user_info(room_num)
                );
                if let Ok(room_data) = room_data
                    && let Ok(user_data) = user_data
                {
                    cx.update_global(|state: &mut AppState, cx| {
                        let room = RoomCard::view(room_data, user_data.info, cx, client);

                        state.room_entities.push(room);
                    })
                    .unwrap();
                };
            })
            .detach();
        }
    }
}

impl Render for LiveRecoderApp {
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
                                                    .child(
                                                        div().flex_1().child(TextInput::new(
                                                            &self.room_input,
                                                        )),
                                                    )
                                                    .child(
                                                        Button::new("添加录制")
                                                            .on_click(
                                                                cx.listener(Self::add_recording),
                                                            )
                                                            .primary()
                                                            .disabled(self.lock)
                                                            .child(Text::String("添加录制".into())),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                // 房间列表卡片
                                div()
                                    .rounded_lg()
                                    .p_4()
                                    .border(gpui::px(1.0))
                                    .border_color(cx.theme().border)
                                    .child(
                                        v_flex()
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
                                                        .gap_3()
                                                        .children(state.room_entities.to_vec())
                                                } else {
                                                    div()
                                                        .p_8()
                                                        .flex_1()
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
