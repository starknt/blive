pub mod room;
pub mod stream;
pub mod user;

#[derive(Debug, serde::Deserialize)]
pub struct BasicResponse<Data: Sized> {
    code: i32,
    data: Data,
}

#[cfg(test)]
mod test {
    use crate::room::get_live_room_info;
    use crate::stream::get_live_room_stream_url;
    use crate::user::get_live_user_info;

    #[tokio::test]
    async fn test_get_live_room_stream_url() {
        let res = get_live_room_stream_url(23767864, 10000).await;

        println!("{res:#?}");
    }

    #[tokio::test]
    async fn test_get_live_user_info() {
        let res = get_live_user_info(23767864).await;

        println!("{res:#?}");
    }

    #[tokio::test]
    async fn test_get_live_room_info() {
        let res = get_live_room_info(23767864).await;

        println!("{res:#?}");
    }
}
