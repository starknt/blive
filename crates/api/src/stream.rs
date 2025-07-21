use reqwest::ClientBuilder;
use serde::Deserialize;

use crate::BasicResponse;

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct PlayUrlInfo {
    conf_json: String,
    playurl: PlayUrl,
}

#[derive(Debug, Deserialize)]
pub struct PlayUrl {
    cid: u64,
    g_qn_desc: Vec<QnDesc>,
    stream: Vec<PlayStream>,
}

#[derive(Debug, Deserialize)]
pub struct QnDesc {
    qn: u32,
    desc: String,
    hdr_desc: String,
    attr_desc: Option<String>,
    hdr_type: u8,
    media_base_desc: Option<MediaBaseDesc>,
}

#[derive(Debug, Deserialize)]
pub struct MediaBaseDesc {
    detail_desc: MediaBaseDescDetail,
    brief_desc: MediaBaseDescBrief,
}

#[derive(Debug, Deserialize)]
pub struct MediaBaseDescDetail {
    desc: String,
}

#[derive(Debug, Deserialize)]
pub struct MediaBaseDescBrief {
    desc: String,
    badge: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlayStream {
    protocol_name: String,
    format: Vec<PlayStreamFormat>,
}

#[derive(Debug, Deserialize)]
pub struct PlayStreamFormat {
    format_name: String,
    codec: Vec<StreamCodec>,
}

#[derive(Debug, Deserialize)]
pub struct StreamCodec {
    codec_name: String,
    current_qn: u32,
    accept_qn: Vec<u32>,
    base_url: String,
    url_info: Vec<StreamUrlInfo>,
}

#[derive(Debug, Deserialize)]
pub struct StreamUrlInfo {
    host: String,
    extra: String,
    stream_ttl: u32,
}

pub async fn get_live_room_stream_url(room_id: u64, qn: u32) -> BasicResponse<LiveRoomStreamUrl> {
    let client = ClientBuilder::new().build().unwrap();

    let response = client.get(format!("https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo?room_id={room_id}&protocol=0,1&format=0,1,2&codec=0,1&qn={qn}"))
      .send()
      .await
      .unwrap();

    let body = response.text().await.unwrap();
    let basic_response: BasicResponse<LiveRoomStreamUrl> = serde_json::from_str(&body).unwrap();

    // let codec = &basic_response.data.playurl_info.playurl.stream[0].format[0].codec[0];
    // let stream_url = format!(
    //     "{}{}{}",
    //     codec.url_info[0].host, codec.base_url, codec.url_info[0].extra
    // );

    // // get stream
    // let mut headers = HeaderMap::new();
    // headers.insert(
    //     REFERER,
    //     HeaderValue::from_static("https://live.bilibili.com/"),
    // );
    // headers.insert(
    //     USER_AGENT,
    //     HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
    // );
    // let mut stream = client
    //     .get(&stream_url)
    //     .headers(headers)
    //     .send()
    //     .await
    //     .unwrap();

    // let mut file = tokio::fs::File::create("recorded.flv").await.unwrap();

    // while let Some(chunk) = stream.chunk().await.unwrap() {
    //     file.write_all(&chunk).await.unwrap();
    // }

    basic_response
}
