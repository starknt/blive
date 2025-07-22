use gpui::{App, Global, SharedString};
use serde::Serialize;
use std::sync::Arc;

use crate::api::{ApiClient, room::LiveRoomInfoData};

#[derive(Serialize)]
pub struct AppState {
    #[serde(skip)]
    pub client: Arc<ApiClient>,
    pub rooms: Vec<LiveRoomInfoData>,
    pub theme_name: Option<SharedString>,
}

impl AppState {
    pub fn init(cx: &mut App) {
        let client = Arc::new(ApiClient::new(cx.http_client()));
        let state = Self {
            client,
            rooms: vec![],
            theme_name: None,
        };
        cx.set_global::<AppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }
}

impl Global for AppState {}
