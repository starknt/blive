use crate::logger::{log_network_request, log_network_response};
use anyhow::{Context, Result};
use futures::AsyncReadExt;
use gpui::http_client::{AsyncBody, HttpClient as GPUIHttpClient, Method, Request, Response};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Instant;

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
        let method = request.method().to_string();
        let url = request.uri().to_string();
        let start_time = Instant::now();

        log_network_request(&url, &method);

        let result = self
            .inner
            .send(request)
            .await
            .context("Failed to send request");

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match &result {
            Ok(response) => {
                log_network_response(response.status().as_u16(), duration_ms);
            }
            Err(_) => {
                log_network_response(0, duration_ms);
            }
        }

        result
    }

    pub async fn get_live_room_info(&self, room_id: u64) -> Result<room::LiveRoomInfoData> {
        let url = format!("https://api.live.bilibili.com/room/v1/Room/get_info?room_id={room_id}");
        let start_time = Instant::now();

        log_network_request(&url, "GET");

        let request = Request::builder()
            .uri(&url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = self
            .inner
            .send(request)
            .await
            .context("Failed to send request")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        log_network_response(response.status().as_u16(), duration_ms);

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
        let start_time = Instant::now();

        log_network_request(&url, "GET");

        let request = Request::builder()
            .uri(&url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = self
            .inner
            .send(request)
            .await
            .context("Failed to send request")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        log_network_response(response.status().as_u16(), duration_ms);

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
        let start_time = Instant::now();

        log_network_request(&url, "GET");

        let request = Request::builder()
            .uri(&url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = self
            .inner
            .send(request)
            .await
            .context("Failed to send request")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        log_network_response(response.status().as_u16(), duration_ms);

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

    async fn get_live_url(
        room_id: u64,
        quality: u32,
        protocol: LiveProtocol,
        container: VideoContainer,
        codec: StreamCodec,
    ) -> Result<String> {
        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let client = HttpClient::new(client);
        let res = client.get_live_room_stream_url(room_id, quality).await;
        assert!(res.is_ok());

        let stream = res.unwrap();
        let playurl_info = stream.playurl_info.unwrap();
        let stream = playurl_info
            .playurl
            .stream
            .iter()
            .find(|stream| stream.protocol_name == protocol)
            .unwrap();
        let stream = stream
            .format
            .iter()
            .find(|f| f.format_name == container)
            .unwrap();
        let stream = stream.codec.iter().find(|c| c.codec_name == codec).unwrap();
        let url_info = &stream.url_info[rand::rng().random_range(0..stream.url_info.len())];
        let url = format!("{}{}{}", url_info.host, stream.base_url, url_info.extra);

        Ok(url)
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_m3u8_file() {
        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let client = HttpClient::new(client);
        let url = get_live_url(
            3044248,
            10000,
            LiveProtocol::HttpHLS,
            VideoContainer::TS,
            StreamCodec::HEVC,
        )
        .await
        .unwrap();

        let mut file = File::create("output/test.m3u8").unwrap();
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
    async fn test_download_http_stream() {
        let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        let client = HttpClient::new(client);
        let url = get_live_url(
            3044248,
            10000,
            LiveProtocol::HttpHLS,
            VideoContainer::TS,
            StreamCodec::HEVC,
        )
        .await
        .unwrap();

        let user_agent_header = format!(
            "User-Agent: {}",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        );
        let referer_header = format!("Referer: {}", "https://live.bilibili.com/");

        let mut response = client
            .send(
                Request::builder()
                    .uri(url)
                    .header("User-Agent", user_agent_header)
                    .header("Referer", referer_header)
                    .body(AsyncBody::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let mut buffer = [0_u8; 8192];
        let mut file = File::create("output/http_stream.flv").unwrap();
        let body = response.body_mut();

        loop {
            let byte_read = body.read(&mut buffer).await.unwrap();
            if byte_read == 0 {
                break;
            }
            file.write_all(&buffer[..byte_read]).unwrap();
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_live_room_stream_url_with_ffmpeg_sidecar() {
        #[cfg(debug_assertions)]
        {
            use ffmpeg_sidecar::command::ffmpeg_is_installed;

            if !ffmpeg_is_installed() {
                ffmpeg_sidecar::download::auto_download().expect("无法自动下载 ffmpeg");
            }
        }

        let url = get_live_url(
            1804892069,
            10000,
            LiveProtocol::HttpHLS,
            VideoContainer::TS,
            StreamCodec::HEVC,
        )
        .await
        .unwrap();

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
            .arg("-bsf:a")
            .arg("aac_adtstoasc") // if using AAC in TS
            .arg("output/ffmpeg_sidecar.mkv");

        let iter = cmd.spawn().unwrap().iter().unwrap();

        for event in iter {
            match event {
                ffmpeg_sidecar::event::FfmpegEvent::Error(err) => {
                    eprintln!("ffmpeg error event: {err:?}");
                }
                ffmpeg_sidecar::event::FfmpegEvent::LogEOF => {
                    println!("ffmpeg log eof");
                }
                _ => {}
            }
        }
    }

    // #[tokio::test]
    // #[ignore]
    // async fn test_get_live_room_stream_url_with_ffmpeg_ez() {
    //     let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
    //     let api_client = HttpClient::new(client);
    //     let res = api_client.get_live_room_stream_url(732, 10000).await;
    //     assert!(res.is_ok());

    //     let stream = res.unwrap();
    //     let playurl_info = stream.playurl_info.unwrap();
    //     let stream = playurl_info
    //         .playurl
    //         .stream
    //         .iter()
    //         .find(|stream| stream.protocol_name == LiveProtocol::HttpHLS)
    //         .unwrap();
    //     let stream = stream
    //         .format
    //         .iter()
    //         .find(|f| f.format_name == VideoContainer::FMP4)
    //         .unwrap();
    //     let stream = stream
    //         .codec
    //         .iter()
    //         .find(|c| c.codec_name == StreamCodec::HEVC)
    //         .unwrap();
    //     let url_info = &stream.url_info[rand::rng().random_range(0..stream.url_info.len())];
    //     let url = format!("{}{}{}", url_info.host, stream.base_url, url_info.extra);

    //     let user_agent_header = format!(
    //         "User-Agent: {}",
    //         "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    //     );
    //     let referer_header = format!("Referer: {}", "https://live.bilibili.com/");

    //     let mut input = ez_ffmpeg::Input::new(url);
    //     input = input
    //         .set_input_opts(vec![
    //             ("user_agent", user_agent_header),
    //             ("referer", referer_header),
    //             ("c", "copy".to_owned()),
    //         ])
    //         .set_video_codec("hevc");

    //     let ctx_builder = ez_ffmpeg::FfmpegContext::builder()
    //         .input(input)
    //         .output(
    //             ez_ffmpeg::Output::new("output/ffmpeg_ez.mkv")
    //                 .set_audio_codec("aac")
    //                 .set_audio_channels(2)
    //                 .set_video_codec("hevc"),
    //         )
    //         .build()
    //         .unwrap();

    //     match ctx_builder.start().unwrap().await {
    //         Ok(()) => {
    //             println!("FFmpeg processing completed successfully!");
    //         }
    //         Err(e) => {
    //             println!("FFmpeg processing failed: {e}");
    //         }
    //     }
    // }

    // #[tokio::test]
    // #[ignore]
    // async fn test_ffmpeg_ez_network_error() {
    //     let client = Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
    //     let api_client = HttpClient::new(client);
    //     let res = api_client.get_live_room_stream_url(721, 10000).await;
    //     assert!(res.is_ok());

    //     let stream = res.unwrap();
    //     let playurl_info = stream.playurl_info.unwrap();
    //     let stream = playurl_info
    //         .playurl
    //         .stream
    //         .iter()
    //         .find(|stream| stream.protocol_name == LiveProtocol::HttpHLS)
    //         .unwrap();
    //     let stream = stream
    //         .format
    //         .iter()
    //         .find(|f| f.format_name == VideoContainer::FMP4)
    //         .unwrap();
    //     let stream = stream
    //         .codec
    //         .iter()
    //         .find(|c| c.codec_name == StreamCodec::HEVC)
    //         .unwrap();
    //     let url_info = &stream.url_info[rand::rng().random_range(0..stream.url_info.len())];
    //     let url = format!("{}{}{}", url_info.host, stream.base_url, url_info.extra);

    //     let user_agent_header = format!(
    //         "User-Agent: {}",
    //         "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    //     );
    //     let referer_header = format!("Referer: {}", "https://live.bilibili.com/");

    //     let mut input = ez_ffmpeg::Input::new(url);
    //     input = input
    //         .set_input_opts(vec![
    //             ("user_agent", user_agent_header),
    //             ("referer", referer_header),
    //             ("c", "copy".to_owned()),
    //             ("reconnect", "0".to_string()),
    //         ])
    //         .set_video_codec("hevc");

    //     let ctx = ez_ffmpeg::FfmpegContext::builder()
    //         .input(input)
    //         .output(
    //             ez_ffmpeg::Output::new("output/ffmpeg_ez_network_error.mkv")
    //                 .set_audio_codec("aac")
    //                 .set_audio_channels(2)
    //                 .set_video_codec("hevc"),
    //         )
    //         .build()
    //         .unwrap();

    //     match ctx.start().unwrap().await {
    //         Ok(_) => {
    //             println!("FFmpeg processing completed successfully!");
    //         }
    //         Err(e) => {
    //             println!("FFmpeg processing failed: {e}");
    //         }
    //     }
    // }

    #[tokio::test]
    #[ignore]
    async fn test_ffmpeg_sidecar_network_error() {
        // 确保 output/ffmpeg_sidecar_network_error.mkv 不存在
        if File::open("output/ffmpeg_sidecar_network_error.mkv").is_ok() {
            std::fs::remove_file("output/ffmpeg_sidecar_network_error.mkv").unwrap();
        }

        ffmpeg_sidecar::download::auto_download().unwrap();

        let url = get_live_url(
            1804892069,
            10000,
            LiveProtocol::HttpHLS,
            VideoContainer::FMP4,
            StreamCodec::AVC,
        )
        .await
        .unwrap();

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
            .arg("-reconnect")
            .arg("1")
            .arg("-reconnect_on_network_error")
            .arg("1")
            .arg("-reconnect_on_http_error")
            .arg("5xx")
            .arg("-reconnect_at_eof")
            .arg("1")
            .arg("-reconnect_streamed")
            .arg("1")
            .arg("-reconnect_delay_max")
            .arg("2")
            .arg("-respect_retry_after")
            .arg("1")
            .arg("-i")
            .arg(url)
            .arg("-xerror")
            .arg("-v")
            .arg("error")
            .arg("-c")
            .arg("copy")
            .arg("-bsf:a")
            .arg("aac_adtstoasc") // if using AAC in TS
            .arg("output/ffmpeg_sidecar_network_error.mkv");

        let mut process = cmd.spawn().unwrap();
        let iter = process.iter().unwrap();

        for event in iter {
            match event {
                ffmpeg_sidecar::event::FfmpegEvent::Error(err) => {
                    eprintln!("ffmpeg error event: {err:?}");
                }
                ffmpeg_sidecar::event::FfmpegEvent::Progress(progress) => {
                    println!("progress: {progress:?}");
                }
                ffmpeg_sidecar::event::FfmpegEvent::LogEOF => {
                    println!("ffmpeg log eof");
                }
                ffmpeg_sidecar::event::FfmpegEvent::Log(_, _) => {}
                _ => {
                    println!("ffmpeg event: {event:?}");
                }
            }
        }

        process.wait().unwrap();
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
