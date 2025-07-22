pub mod api;
pub mod assets;
pub mod components;
pub mod state;
pub mod themes;
pub mod title_bar;

use std::sync::Arc;

use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Root,
    button::Button,
    h_flex,
    input::{InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
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
        let room_num = 0;
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

    fn add_recording(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.room_num > 0 {
            let client = Arc::clone(&AppState::global_mut(cx).client);
            let room_num = self.room_num;

            cx.spawn(async move |_, cx| {
                if let Ok(data) = client.get_live_room_info(room_num).await {
                    let _ = cx.update_global(|state: &mut AppState, _| {
                        state.rooms.push(data);
                    });
                };
            })
            .detach();

            self.room_num = 0;
            self.room_input.update(cx, |input, cx| {
                input.set_value(self.room_num.to_string(), window, cx);
            });
        }
    }
}

impl Render for LiveRecoderApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                    .border_color(gpui::rgb(0xe2e8f0))
                                    .child(
                                        v_flex()
                                            .gap_4()
                                            .child(Text::String("添加录制房间".into()))
                                            .child(
                                                h_flex()
                                                    .gap_3()
                                                    .items_center()
                                                    .child(
                                                        div().flex_1().child(NumberInput::new(
                                                            &self.room_input,
                                                        )),
                                                    )
                                                    .child(
                                                        Button::new("添加录制")
                                                            .on_click(
                                                                cx.listener(Self::add_recording),
                                                            )
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
                                    .border_color(gpui::rgb(0xe2e8f0))
                                    .child(
                                        v_flex()
                                            .gap_4()
                                            .child(
                                                h_flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(Text::String("录制房间列表".into()))
                                                    .child(Text::String(
                                                        format!("共 {} 个房间", state.rooms.len())
                                                            .into(),
                                                    )),
                                            )
                                            .child(v_flex().gap_3().children({
                                                let rooms: Vec<_> = state.rooms.to_vec();
                                                rooms
                                                    .into_iter()
                                                    .map(|room| cx.new(|_| RoomCard::new(room)))
                                                    .collect::<Vec<_>>()
                                            })),
                                    ),
                            ),
                    )
                    .child(div().absolute().top_8().children(notification_layer)),
            ),
        )
    }
}
