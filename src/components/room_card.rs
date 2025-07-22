use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants},
    text::Text,
    *,
};

use crate::api::room::{LiveRoomInfoData, LiveStatus};

// #[derive(Clone)]
// pub enum RoomCardEvent {}

// #[derive(Clone, PartialEq, Debug)]
// pub enum RoomStatus {
//     Waiting,
//     Recording,
//     Error,
// }

pub struct RoomCard {
    pub(super) room: u64,
    pub(super) title: String,
    pub(super) description: String,
    pub(super) cover_url: String,
    pub(super) live_status: LiveStatus,
    pub(super) online_count: u32,
    pub(super) area_name: String,
}

impl RoomCard {
    pub fn new(room: LiveRoomInfoData) -> Self {
        Self {
            room: room.room_id,
            title: room.title,
            description: room.description,
            cover_url: room.user_cover,
            live_status: room.live_status,
            online_count: room.online,
            area_name: room.area_name,
        }
    }
}

impl Render for RoomCard {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .rounded_lg()
            .p_4()
            .border(px(1.0))
            .border_color(gpui::rgb(0xe2e8f0))
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
                                        div()
                                            .w_16()
                                            .h_12()
                                            .rounded_md()
                                            .bg(gpui::rgb(0xf1f5f9))
                                            .child(
                                                div()
                                                    .size_full()
                                                    .bg(gpui::rgb(0x94a3b8))
                                                    .rounded_md()
                                                    .child(
                                                        img(self.cover_url.clone())
                                                            .block()
                                                            .max_w_20()
                                                            .size_full()
                                                            .object_fit(ObjectFit::Cover),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        // 房间信息
                                        v_flex()
                                            .gap_1()
                                            .child(self.title.clone().into_element())
                                            .child(format!("房间号: {}", self.room).into_element())
                                            .child(
                                                // 直播状态和在线人数
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        // 直播状态指示器
                                                        div().w_2().h_2().rounded_full().bg(
                                                            if self.live_status == LiveStatus::Live
                                                            {
                                                                gpui::rgb(0xef4444)
                                                            } else {
                                                                gpui::rgb(0x6b7280)
                                                            },
                                                        ),
                                                    )
                                                    .child(Text::String(
                                                        if self.live_status == LiveStatus::Live {
                                                            "直播中".into()
                                                        } else {
                                                            "未开播".into()
                                                        },
                                                    ))
                                                    .child(
                                                        format!("{} 人观看", self.online_count)
                                                            .into_element(),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                // 右侧：操作按钮
                                v_flex()
                                    .gap_2()
                                    .child(Button::new("开始录制").primary().label("开始录制"))
                                    .child(Button::new("删除").danger().label("删除")),
                            ),
                    )
                    .child(div().child(self.description.clone().into_element()))
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child("分区: ".into_element())
                            .child(self.area_name.clone().into_element()),
                    ),
            )
    }
}
