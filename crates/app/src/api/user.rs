use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct LiveUserData {
    info: LiveUserInfo,
    level: LiveUserLevel,
    san: i32,
}

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
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
