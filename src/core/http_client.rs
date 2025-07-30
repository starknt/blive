use anyhow::{Context, Result};
use futures_util::AsyncReadExt;
use gpui::http_client::{AsyncBody, HttpClient as GPUIHttpClient, Method, Request, Response};
use std::fmt::Debug;
use std::sync::Arc;

pub mod room;
pub mod stream;
pub mod user;

#[derive(Debug, serde::Deserialize)]
pub struct BasicResponse<Data: Sized> {
    pub code: i32,
    pub data: Data,
}

pub struct HttpClient {
    inner: Arc<dyn GPUIHttpClient>,
}

impl HttpClient {
    pub fn new(client: Arc<dyn GPUIHttpClient>) -> Self {
        Self { inner: client }
    }

    pub async fn send(&self, request: Request<AsyncBody>) -> Result<Response<AsyncBody>> {
        self.inner
            .send(request)
            .await
            .context("Failed to send request")
    }

    pub async fn get_live_room_info(&self, room_id: u64) -> Result<room::LiveRoomInfoData> {
        let url = format!("https://api.live.bilibili.com/room/v1/Room/get_info?room_id={room_id}");

        let request = Request::builder()
            .uri(url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = self
            .inner
            .send(request)
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get live room info"));
        }
        let mut body = String::new();
        response.body_mut().read_to_string(&mut body).await?;

        let data: BasicResponse<room::LiveRoomInfoData> = serde_json::from_str(&body)?;

        Ok(data.data)
    }

    pub async fn get_live_room_stream_url(
        &self,
        room_id: u64,
        quality: u32,
    ) -> Result<stream::LiveRoomStreamUrl> {
        let url = format!(
            "https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo?room_id={room_id}&protocol=0,1&format=0,1,2&codec=0,1&qn={quality}"
        );

        let request = Request::builder()
            .uri(url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = self
            .inner
            .send(request)
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get live room stream url"));
        }

        let mut body = String::new();
        response.body_mut().read_to_string(&mut body).await?;

        let data: BasicResponse<stream::LiveRoomStreamUrl> = serde_json::from_str(&body)?;

        Ok(data.data)
    }

    pub async fn get_live_room_user_info(&self, room_id: u64) -> Result<user::LiveUserData> {
        let url = format!(
            "https://api.live.bilibili.com/live_user/v1/UserInfo/get_anchor_in_room?roomid={room_id}"
        );

        let request = Request::builder()
            .uri(url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = self
            .inner
            .send(request)
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get live room user info"));
        }

        let mut body = String::new();
        response.body_mut().read_to_string(&mut body).await?;

        let data: BasicResponse<user::LiveUserData> = serde_json::from_str(&body)?;

        Ok(data.data)
    }
}

impl Clone for HttpClient {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HttpClient")
    }
}

#[cfg(test)]
mod test {
    use crate::settings::{LiveProtocol, StreamCodec, VideoContainer};

    use super::*;
    use ffmpeg_sidecar::command::FfmpegCommand;
    use rand::Rng;
    use reqwest_client::ReqwestClient;
    use std::{fs::File, io::Write, sync::Arc};

    #[tokio::test]
    #[ignore]
    async fn test_download_m3u8_file() {
        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let client = HttpClient::new(client);
        let res = client.get_live_room_stream_url(3044248, 10000).await;

        let stream = res.unwrap();
        let playurl_info = stream.playurl_info.unwrap();
        let stream = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == LiveProtocol::HttpHLS)
            .unwrap();
        let stream = stream
            .format
            .iter()
            .find(|f| f.format_name == VideoContainer::TS)
            .unwrap();
        let stream = stream
            .codec
            .iter()
            .find(|c| c.codec_name == StreamCodec::HEVC)
            .unwrap();
        let url_info = &stream.url_info[rand::rng().random_range(0..stream.url_info.len())];
        let url = format!("{}{}{}", url_info.host, stream.base_url, url_info.extra);
        println!("url: {url}");
        let mut file = File::create("test.m3u8").unwrap();
        let mut response = client
            .send(
                Request::builder()
                    .uri(url)
                    .body(AsyncBody::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let mut body = String::new();
        response.body_mut().read_to_string(&mut body).await.unwrap();
        file.write_all(body.as_bytes()).unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_live_room_stream_url() {
        ffmpeg_sidecar::download::auto_download().unwrap();

        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let api_client = HttpClient::new(client);
        let res = api_client.get_live_room_stream_url(1804892069, 10000).await;
        println!("{res:#?}");
        assert!(res.is_ok());

        let stream = res.unwrap();
        let playurl_info = stream.playurl_info.unwrap();
        let stream = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == LiveProtocol::HttpHLS)
            .unwrap();
        let stream = stream
            .format
            .iter()
            .find(|f| f.format_name == VideoContainer::FMP4)
            .unwrap();
        let stream = stream
            .codec
            .iter()
            .find(|c| c.codec_name == StreamCodec::AVC)
            .unwrap();
        let url_info = &stream.url_info[rand::rng().random_range(0..stream.url_info.len())];
        let url = format!("{}{}{}", url_info.host, stream.base_url, url_info.extra);

        let user_agent_header = format!(
            "User-Agent: {}",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        );
        let referer_header = format!("Referer: {}", "https://live.bilibili.com/");

        let mut cmd = FfmpegCommand::new();
        cmd.arg("-headers")
            .arg(user_agent_header)
            .arg("-headers")
            .arg(referer_header)
            .arg("-i")
            .arg(url)
            .arg("-c")
            .arg("copy")
            // .arg("-bsf:a")
            // .arg("aac_adtstoasc") // if using AAC in TS
            .arg("test.mkv");

        let iter = cmd.spawn().unwrap().iter().unwrap();

        for frame in iter.filter_frames() {
            println!("frame: {}x{}", frame.width, frame.height);
            let _pixels: Vec<u8> = frame.data; // <- raw RGB pixels! 🎨
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_live_room_stream_url_with_ffmpeg_ez() {
        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let api_client = HttpClient::new(client);
        let res = api_client.get_live_room_stream_url(732, 10000).await;
        assert!(res.is_ok());

        let stream = res.unwrap();
        let playurl_info = stream.playurl_info.unwrap();
        let stream = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == LiveProtocol::HttpHLS)
            .unwrap();
        let stream = stream
            .format
            .iter()
            .find(|f| f.format_name == VideoContainer::FMP4)
            .unwrap();
        let stream = stream
            .codec
            .iter()
            .find(|c| c.codec_name == StreamCodec::HEVC)
            .unwrap();
        let url_info = &stream.url_info[rand::rng().random_range(0..stream.url_info.len())];
        let url = format!("{}{}{}", url_info.host, stream.base_url, url_info.extra);

        let user_agent_header = format!(
            "User-Agent: {}",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        );
        let referer_header = format!("Referer: {}", "https://live.bilibili.com/");

        let mut input = ez_ffmpeg::Input::new(url);
        input = input
            .set_input_opts(vec![
                ("user_agent", user_agent_header),
                ("referer", referer_header),
                ("c", "copy".to_owned()),
            ])
            .set_video_codec("hevc");

        let ctx_builder = ez_ffmpeg::FfmpegContext::builder()
            .input(input)
            .output(
                ez_ffmpeg::Output::new("test2.mkv")
                    .set_audio_codec("aac")
                    .set_audio_channels(2)
                    .set_video_codec("hevc"),
            )
            .build()
            .unwrap();

        match ctx_builder.start().unwrap().await {
            Ok(()) => {
                println!("FFmpeg processing completed successfully!");
            }
            Err(e) => {
                println!("FFmpeg processing failed: {e}");
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_ffmpeg_ez_network_error() {
        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let api_client = HttpClient::new(client);
        let res = api_client.get_live_room_stream_url(721, 10000).await;
        assert!(res.is_ok());

        let stream = res.unwrap();
        let playurl_info = stream.playurl_info.unwrap();
        let stream = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == LiveProtocol::HttpHLS)
            .unwrap();
        let stream = stream
            .format
            .iter()
            .find(|f| f.format_name == VideoContainer::FMP4)
            .unwrap();
        let stream = stream
            .codec
            .iter()
            .find(|c| c.codec_name == StreamCodec::HEVC)
            .unwrap();
        let url_info = &stream.url_info[rand::rng().random_range(0..stream.url_info.len())];
        let url = format!("{}{}{}", url_info.host, stream.base_url, url_info.extra);

        let user_agent_header = format!(
            "User-Agent: {}",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        );
        let referer_header = format!("Referer: {}", "https://live.bilibili.com/");

        let mut input = ez_ffmpeg::Input::new(url);
        input = input
            .set_input_opts(vec![
                ("user_agent", user_agent_header),
                ("referer", referer_header),
                ("c", "copy".to_owned()),
                ("reconnect", "0".to_string()),
            ])
            .set_video_codec("hevc");

        let ctx = ez_ffmpeg::FfmpegContext::builder()
            .input(input)
            .output(
                ez_ffmpeg::Output::new("test2.mkv")
                    .set_audio_codec("aac")
                    .set_audio_channels(2)
                    .set_video_codec("hevc"),
            )
            .build()
            .unwrap();

        match ctx.start().unwrap().await {
            Ok(_) => {
                println!("FFmpeg processing completed successfully!");
            }
            Err(e) => {
                println!("FFmpeg processing failed: {e}");
            }
        }
    }

    #[tokio::test]
    async fn test_get_live_user_info() {
        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let api_client = HttpClient::new(client);
        let res = api_client.get_live_room_user_info(1804892069).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_get_live_room_info() {
        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let api_client = HttpClient::new(client);
        let res = api_client.get_live_room_info(1804892069).await;
        assert!(res.is_ok());
    }
}
