use gpui::{App, Entity, EventEmitter, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState, TextInput},
    v_flex,
};

use crate::state::AppState;

#[derive(Debug, Clone)]
pub enum RoomInputEvent {
    RoomInputSubmit(u64),
}

pub struct RoomInput {
    room_id: u64,
    valid: bool,
    room_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl RoomInput {
    fn new(room_id: u64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let room_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("请输入直播间房间号")
                .pattern(regex::Regex::new(r"^\d+$").unwrap())
                .default_value(room_id.to_string())
        });

        let _subscriptions = vec![cx.subscribe_in(&room_input, window, Self::on_room_input_change)];

        Self {
            valid: false,
            room_id,
            room_input,
            _subscriptions,
        }
    }

    pub fn view(room_id: u64, window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(room_id, window, cx))
    }

    fn on_room_input_change(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change(text) = event
            && let Ok(value) = text.parse::<u64>()
        {
            let room_id = value;
            self.room_id = value;
            // Reset validity when input changes
            self.valid = false;

            // check the room id is valid
            cx.spawn_in(window, async move |this, cx| {
                if let Ok(client) = cx.read_global(|state: &AppState, _, _| state.client.clone()) {
                    if client.get_live_room_info(room_id).await.is_ok() {
                        if let Some(entity) = this.upgrade() {
                            let _ = entity.update(cx, |this, _| {
                                this.valid = true;
                            });
                        }
                    }
                }
            })
            .detach();
        }
    }
}

impl EventEmitter<RoomInputEvent> for RoomInput {}

impl Render for RoomInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(div().font_bold().text_lg().child("添加录制房间"))
                    .child(
                        div().rounded_lg().p_4().bg(cx.theme().background).child(
                            v_flex()
                                .gap_4()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().accent_foreground)
                                        .child("请输入B站直播间房间号"),
                                )
                                .child(
                                    h_flex()
                                        .max_w_96()
                                        .gap_4()
                                        .items_center()
                                        .child(
                                            div()
                                                .flex_1()
                                                .rounded_lg()
                                                .border(px(1.0))
                                                .border_color(cx.theme().border)
                                                .bg(cx.theme().background)
                                                .child(
                                                    TextInput::new(&self.room_input)
                                                        .p_3()
                                                        .text_lg(),
                                                ),
                                        )
                                        .child(
                                            Button::new("添加录制")
                                                .label("添加录制")
                                                .primary()
                                                .disabled(!self.valid)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    cx.emit(RoomInputEvent::RoomInputSubmit(
                                                        this.room_id,
                                                    ));
                                                })),
                                        ),
                                ),
                        ),
                    ),
            )
    }
}
