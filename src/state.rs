use gpui::{App, Entity, Global};
use std::sync::Arc;

use crate::{api::ApiClient, components::RoomCard, settings::GlobalSettings};

pub struct AppState {
    pub client: Arc<ApiClient>,
    pub room_entities: Vec<Entity<RoomCard>>,
    pub settings: GlobalSettings,
}

impl AppState {
    pub fn init(cx: &mut App) {
        let client = Arc::new(ApiClient::new(cx.http_client()));
        let state = Self {
            client,
            room_entities: vec![],
            settings: GlobalSettings::load(),
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
