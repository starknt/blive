use gpui::{App, Entity, EventEmitter, Subscription, Window, div, prelude::*};
use gpui_component::input::{InputEvent, InputState, TextInput};

#[derive(Debug, Clone)]
pub enum RoomInputEvent {
    RoomInputChange(u64),
}

pub struct RoomInput {
    room_input: Entity<InputState>,
    room_num: u64,
    _subscriptions: Vec<Subscription>,
}

impl RoomInput {
    fn new(room_num: u64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let room_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("请输入直播间房间号")
                .pattern(regex::Regex::new(r"^\d+$").unwrap())
                .default_value(room_num.to_string())
        });

        let _subscriptions = vec![cx.subscribe_in(&room_input, window, Self::on_room_input_change)];

        Self {
            room_num,
            room_input,
            _subscriptions,
        }
    }

    pub fn view(room_num: u64, window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(room_num, window, cx))
    }

    fn on_room_input_change(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change(text) = event
            && let Ok(value) = text.parse::<u64>()
        {
            self.room_num = value;

            cx.emit(RoomInputEvent::RoomInputChange(self.room_num));
        }
    }
}

impl EventEmitter<RoomInputEvent> for RoomInput {}

impl Render for RoomInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().flex_1().child(TextInput::new(&self.room_input))
    }
}
