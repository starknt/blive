use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct LiveRoomStreamUrl {
    room_id: u64,
    short_id: u64,
    uid: u64,
    is_hidden: bool,
    is_locked: bool,
    is_portrait: bool,
    live_status: u8,
    hidden_till: u64,
    lock_till: u64,
    encrypted: bool,
    pwd_verified: bool,
    live_time: u64,
    room_shield: u8,
    all_special_types: Vec<u8>,
    playurl_info: Option<PlayUrlInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlayUrlInfo {
    conf_json: String,
    playurl: PlayUrl,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlayUrl {
    cid: u64,
    g_qn_desc: Vec<QnDesc>,
    stream: Vec<PlayStream>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QnDesc {
    qn: u32,
    desc: String,
    hdr_desc: String,
    attr_desc: Option<String>,
    hdr_type: u8,
    media_base_desc: Option<MediaBaseDesc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MediaBaseDesc {
    detail_desc: MediaBaseDescDetail,
    brief_desc: MediaBaseDescBrief,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MediaBaseDescDetail {
    desc: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MediaBaseDescBrief {
    desc: String,
    badge: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlayStream {
    protocol_name: String,
    format: Vec<PlayStreamFormat>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlayStreamFormat {
    format_name: String,
    codec: Vec<StreamCodec>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StreamCodec {
    codec_name: String,
    current_qn: u32,
    accept_qn: Vec<u32>,
    base_url: String,
    url_info: Vec<StreamUrlInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StreamUrlInfo {
    host: String,
    extra: String,
    stream_ttl: u32,
}
