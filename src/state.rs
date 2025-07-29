use crate::{components::RoomCard, core::HttpClient, settings::GlobalSettings};
use gpui::{App, Entity, Global};

pub struct AppState {
    pub client: HttpClient,
    pub room_entities: Vec<Entity<RoomCard>>,
    pub settings: GlobalSettings,
}

impl AppState {
    pub fn init(cx: &mut App) {
        let client = HttpClient::new(cx.http_client());
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
