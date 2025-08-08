use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct LiveUserData {
    pub info: LiveUserInfo,
    pub level: LiveUserLevel,
    pub san: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct LiveUserInfo {
    pub uid: u64,
    pub uname: String,
    pub face: String,
    pub rank: String,
    pub platform_user_level: u8,
    pub mobile_verify: i8,
    pub identification: i8,
    pub vip_type: i8,
    pub gender: i8,
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
