use num_enum::FromPrimitive;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromPrimitive, PartialEq, Eq, Default)]
#[repr(u8)]
#[serde(from = "u8")]
pub enum LiveStatus {
    #[default]
    Offline = 0,
    Live = 1,
    Carousel = 2,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct LiveRoomInfoData {
    pub uid: u64,
    pub room_id: u64,
    pub short_id: u64,
    pub attention: u32,
    pub online: u32,
    pub is_portrait: bool,
    pub description: String,
    pub live_status: LiveStatus,
    pub parent_area_id: u32,
    pub parent_area_name: String,
    pub old_area_id: u32,
    pub background: String,
    pub title: String,
    pub user_cover: String,
    pub live_time: String,
    pub tags: String,
    pub area_name: String,
}
