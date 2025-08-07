use crate::core::downloader::BLiveDownloader;
use crate::logger::{log_config_change, log_user_action};
use crate::{components::RoomCard, core::HttpClient, settings::GlobalSettings};
use gpui::{App, Entity, Global};
use std::sync::Arc;

pub struct AppState {
    pub client: HttpClient,
    pub room_entities: Vec<Entity<RoomCard>>,
    pub settings: GlobalSettings,
    pub downloaders: Vec<Arc<BLiveDownloader>>,
}

impl AppState {
    pub fn init(cx: &mut App) {
        log_user_action("初始化应用状态", None);

        let client = HttpClient::new(cx.http_client());
        let settings = GlobalSettings::load();

        log_config_change("录制目录", &settings.record_dir);
        log_config_change("默认录制质量", &format!("{}", settings.quality));
        log_config_change("默认录制格式", &format!("{}", settings.format));
        log_config_change("默认编码格式", &format!("{}", settings.codec));
        log_config_change("主题", &settings.theme_name);

        if !settings.rooms.is_empty() {
            log_user_action(
                "加载已保存的房间",
                Some(&format!("共{}个房间", settings.rooms.len())),
            );
        }

        let state = Self {
            client,
            settings,
            room_entities: vec![],
            downloaders: vec![],
        };
        cx.set_global::<AppState>(state);

        log_user_action("应用状态初始化完成", None);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }
}

impl Global for AppState {}
