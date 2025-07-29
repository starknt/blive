use serde::{Deserialize, Serialize};

use crate::settings::{StreamCodec, VideoContainer};

#[derive(Debug, Deserialize, Serialize)]
pub struct LiveRoomStreamUrl {
    pub room_id: u64,
    pub short_id: u64,
    pub uid: u64,
    pub is_hidden: bool,
    pub is_locked: bool,
    pub is_portrait: bool,
    pub live_status: u8,
    pub hidden_till: u64,
    pub lock_till: u64,
    pub encrypted: bool,
    pub pwd_verified: bool,
    pub live_time: u64,
    pub room_shield: u8,
    pub all_special_types: Vec<u8>,
    pub playurl_info: Option<PlayUrlInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlayUrlInfo {
    pub conf_json: String,
    pub playurl: PlayUrl,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlayUrl {
    pub cid: u64,
    pub g_qn_desc: Vec<QnDesc>,
    pub stream: Vec<PlayStream>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QnDesc {
    pub qn: u32,
    pub desc: String,
    pub hdr_desc: String,
    pub attr_desc: Option<String>,
    pub hdr_type: u8,
    pub media_base_desc: Option<MediaBaseDesc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MediaBaseDesc {
    pub detail_desc: MediaBaseDescDetail,
    pub brief_desc: MediaBaseDescBrief,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MediaBaseDescDetail {
    pub desc: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MediaBaseDescBrief {
    pub desc: String,
    pub badge: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlayStream {
    pub protocol_name: String,
    pub format: Vec<PlayStreamFormat>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlayStreamFormat {
    pub format_name: VideoContainer,
    pub codec: Vec<StreamCodecInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StreamCodecInfo {
    pub codec_name: StreamCodec,
    pub current_qn: u32,
    pub accept_qn: Vec<u32>,
    pub base_url: String,
    pub url_info: Vec<StreamUrlInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StreamUrlInfo {
    pub host: String,
    pub extra: String,
    pub stream_ttl: u32,
}
