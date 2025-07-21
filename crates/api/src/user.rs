use serde::Deserialize;

use crate::BasicResponse;

#[derive(Debug, Deserialize)]
pub struct LiveUserData {
    info: LiveUserInfo,
    level: LiveUserLevel,
    san: i32,
}

#[derive(Debug, Deserialize)]
pub struct LiveUserInfo {
    uid: u64,
    uname: String,
    face: String,
    rank: String,
    platform_user_level: u8,
    mobile_verify: i8,
    identification: i8,
    vip_type: i8,
    gender: i8,
}

#[derive(Debug, Deserialize)]
pub struct LiveUserLevel {
    uid: u64,
    cost: u64,
    rcost: u64,
    user_score: String,
    vip: u8,
    vip_time: String,
    svip_time: String,
    update_time: String,
}

pub async fn get_live_user_info(room_id: u64) -> BasicResponse<LiveUserData> {
    let response = reqwest::get(format!(
        "https://api.live.bilibili.com/live_user/v1/UserInfo/get_anchor_in_room?roomid={room_id}"
    ))
    .await
    .unwrap();

    let body = response.text().await.unwrap();
    let live_user_data: BasicResponse<LiveUserData> = serde_json::from_str(&body).unwrap();

    live_user_data
}
