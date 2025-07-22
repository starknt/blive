use anyhow::Context;
use anyhow::Result;
use futures::AsyncReadExt;
use gpui::http_client::{AsyncBody, HttpClient, Method, Request};
use std::sync::Arc;

pub mod room;
pub mod stream;
pub mod user;

#[derive(Debug, serde::Deserialize)]
pub struct BasicResponse<Data: Sized> {
    pub code: i32,
    pub data: Data,
}

pub struct ApiClient {
    client: Arc<dyn HttpClient>,
}

impl ApiClient {
    pub fn new(client: Arc<dyn HttpClient>) -> Self {
        Self { client }
    }

    pub async fn get_live_room_info(&self, room_id: u64) -> Result<room::LiveRoomInfoData> {
        let url = format!("https://api.live.bilibili.com/room/v1/Room/get_info?room_id={room_id}");

        let request = Request::builder()
            .uri(url)
            .method(Method::GET)
            .body(AsyncBody::empty())
            .context("Failed to build request")?;

        let mut response = self
            .client
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
            .client
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
            .client
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

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_get_live_room_stream_url() {
        let client =
            Arc::new(reqwest_client::ReqwestClient::user_agent("LiveRecorder/0.1.0").unwrap());
        let api_client = ApiClient::new(client);
        let res = api_client.get_live_room_stream_url(1804892069, 10000).await;

        println!("{res:#?}");
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_get_live_user_info() {
        let client =
            Arc::new(reqwest_client::ReqwestClient::user_agent("LiveRecorder/0.1.0").unwrap());
        let api_client = ApiClient::new(client);
        let res = api_client.get_live_room_user_info(1804892069).await;

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_get_live_room_info() {
        let client =
            Arc::new(reqwest_client::ReqwestClient::user_agent("LiveRecorder/0.1.0").unwrap());
        let api_client = ApiClient::new(client);
        let res = api_client.get_live_room_info(1804892069).await;

        assert!(res.is_ok());
    }
}
