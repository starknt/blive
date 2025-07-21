use serde::Deserialize;

use crate::BasicResponse;

#[derive(Debug, Deserialize)]
pub struct LiveRoomInfoData {
    uid: u64,
    room_id: u64,
    short_id: u64,
    attention: u32,
    online: u32,
    is_portrait: bool,
    description: String,
    live_status: u8,
    parent_area_id: u32,
    parent_area_name: String,
    old_area_id: u32,
    background: String,
    title: String,
    user_cover: String,
    live_time: String,
    tags: String,
    area_name: String,
    hot_words: Vec<String>,
}

pub async fn get_live_room_info(
    room_id: u64,
) -> Result<BasicResponse<LiveRoomInfoData>, Box<dyn std::error::Error>> {
    let url: String =
        format!("https://api.live.bilibili.com/room/v1/Room/get_info?room_id={room_id}",);

    let res = reqwest::get(url).await?;

    let body = res.text().await?;

    let live_room_info_response: BasicResponse<LiveRoomInfoData> = serde_json::from_str(&body)?;

    Ok(live_room_info_response)
}
