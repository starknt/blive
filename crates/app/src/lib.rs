pub mod assets;
pub mod title_bar;

use gpui::{prelude::*, *};
use gpui_component::{
    Root,
    button::Button,
    h_flex,
    input::{InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    text::Text,
    v_flex,
};

use crate::title_bar::AppTitleBar;

pub struct LiveRecorderApp {
    room_num: u64,
    room_input: Entity<InputState>,
    title_bar: Entity<AppTitleBar>,
    _subscriptions: Vec<Subscription>,
}

pub struct RoomRecorder {
    pub num: u64,
    pub status: RoomStatus,
}

#[derive(Clone, PartialEq)]
pub enum RoomStatus {
    Waiting,
    Recording,
    Error,
}

impl RoomRecorder {
    pub fn new(num: u64) -> Self {
        Self {
            num,
            status: RoomStatus::Waiting,
        }
    }
}

pub struct LiveRecorderAppState {
    pub rooms: Vec<RoomRecorder>,
    pub theme_name: Option<SharedString>,
}

impl LiveRecorderAppState {
    pub fn init(cx: &mut App) {
        let state = Self {
            rooms: vec![],
            theme_name: None,
        };
        cx.set_global::<LiveRecorderAppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn add_room(&mut self, room_num: u64) {
        // 检查房间是否已存在
        if !self.rooms.iter().any(|room| room.num == room_num) {
            self.rooms.push(RoomRecorder::new(room_num));
        }
    }

    pub fn remove_room(&mut self, room_num: u64) {
        self.rooms.retain(|room| room.num != room_num);
    }
}

impl Global for LiveRecorderAppState {}

impl LiveRecorderApp {
    fn on_room_input_change(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change(text) => {
                if let Ok(value) = text.parse::<u64>() {
                    self.room_num = value;
                }
                println!("Change text: {text}");
            }
            InputEvent::PressEnter { secondary } => {
                println!("PressEnter secondary: {secondary}");
            }
            InputEvent::Focus => println!("Focus"),
            InputEvent::Blur => println!("Blur"),
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

impl LiveRecorderApp {
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
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn add_recording(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.room_num > 0 {
            LiveRecorderAppState::global_mut(cx).add_room(self.room_num);
            // 清空输入框
            self.room_num = 0;
            self.room_input.update(cx, |input, cx| {
                input.set_value(self.room_num.to_string(), window, cx);
            });
            cx.notify();
        }
    }
}

impl Render for LiveRecorderApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notification_layer = Root::render_notification_layer(window, cx);
        let state = LiveRecorderAppState::global(cx);

        div().size_full().child(
            v_flex()
                .size_full()
                .child(self.title_bar.clone())
                .child(
                    div().flex_1().p_6().overflow_hidden().child(
                        v_flex()
                            .justify_start()
                            .gap_6()
                            .child(
                                // 输入区域卡片
                                div()
                                    .bg(gpui::rgb(0xf8fafc))
                                    .rounded_lg()
                                    .p_4()
                                    .border(gpui::px(1.0))
                                    .border_color(gpui::rgb(0xe2e8f0))
                                    .child(
                                        v_flex()
                                            .gap_4()
                                            .child(
                                                Text::String("添加录制房间".into())
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_3()
                                                    .items_center()
                                                    .child(
                                                        div().flex_1().child(
                                                            NumberInput::new(&self.room_input)
                                                        )
                                                    )
                                                    .child(
                                                        Button::new("添加录制")
                                                            .on_click(cx.listener(Self::add_recording))
                                                            .child(Text::String("添加录制".into()))
                                                    )
                                            )
                                    )
                            )
                            .child(
                                // 房间列表卡片
                                div()
                                    .bg(gpui::rgb(0xf8fafc))
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
                                                    .child(
                                                        Text::String("录制房间列表".into())
                                                    )
                                                    .child(
                                                        Text::String(format!("共 {} 个房间", state.rooms.len()).into())
                                                    )
                                            )
                                            .child(
                                                if state.rooms.is_empty() {
                                                    div()
                                                        .p_8()
                                                        .child(
                                                            Text::String("暂无录制房间，请添加房间号开始录制".into())
                                                        )
                                                } else {
                                                    v_flex()
                                                        .gap_3()
                                                        .children(state.rooms.iter().map(|room| {
                                                            div()
                                                                .bg(gpui::white())
                                                                .rounded_md()
                                                                .p_3()
                                                                .border(gpui::px(1.0))
                                                                .border_color(gpui::rgb(0xe2e8f0))
                                                                .child(
                                                                    h_flex()
                                                                        .justify_between()
                                                                        .items_center()
                                                                        .child(
                                                                            h_flex()
                                                                                .gap_3()
                                                                                .items_center()
                                                                                .child(
                                                                                    div()
                                                                                        .w_3()
                                                                                        .h_3()
                                                                                        .rounded_full()
                                                                                        .bg(match room.status {
                                                                                            RoomStatus::Waiting => gpui::rgb(0xfbbf24),
                                                                                            RoomStatus::Recording => gpui::rgb(0x10b981),
                                                                                            RoomStatus::Error => gpui::rgb(0xef4444),
                                                                                        })
                                                                                )
                                                                                .child(
                                                                                    Text::String(format!("房间号: {}", room.num).into())
                                                                                )
                                                                        )
                                                                        .child(
                                                                            Button::new("删除")
                                                                                .child(Text::String("删除".into()))
                                                                        )
                                                                )
                                                        }))
                                                }
                                            )
                                    )
                            )
                    ),
                )
                .child(div().absolute().top_8().children(notification_layer)),
        )
    }
}
