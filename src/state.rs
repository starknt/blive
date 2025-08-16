use crate::components::{DownloaderStatus, RoomCard, RoomCardStatus};
use crate::core::downloader::BLiveDownloader;
use crate::core::http_client::room::LiveRoomInfoData;
use crate::core::http_client::user::LiveUserInfo;
use crate::logger::{log_config_change, log_user_action};
use crate::settings::RoomSettings;
use crate::{core::HttpClient, settings::GlobalSettings};
use gpui::{App, Global, WeakEntity};
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct RoomCardState {
    pub room_id: u64,
    pub status: RoomCardStatus,
    pub user_stop: bool,
    pub(crate) room_info: Option<LiveRoomInfoData>,
    pub(crate) user_info: Option<LiveUserInfo>,
    pub downloader: Option<Arc<BLiveDownloader>>,
    pub downloader_status: Option<DownloaderStatus>,
    pub reconnecting: bool,
    pub reconnect_manager: ReconnectManager,
    pub entity: Option<WeakEntity<RoomCard>>,
}

#[derive(Debug, Clone, Default)]
pub struct ReconnectManager {
    current_attempt: u32,
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    last_reconnect_time: Option<std::time::Instant>,
}

impl ReconnectManager {
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            current_attempt: 0,
            max_attempts,
            base_delay,
            max_delay,
            last_reconnect_time: None,
        }
    }

    pub fn should_reconnect(&self) -> bool {
        self.current_attempt < self.max_attempts
    }

    pub fn increment_attempt(&mut self) {
        self.current_attempt += 1;
        self.last_reconnect_time = Some(std::time::Instant::now());
    }

    pub fn calculate_delay(&self) -> Duration {
        // 指数退避算法，带随机抖动
        let exponential_delay = self.base_delay * (2_u32.pow(self.current_attempt.min(10)));
        let jitter = rand::rng().random_range(0.8..1.2);

        let delay = Duration::from_secs_f64(exponential_delay.as_secs_f64() * jitter);

        delay.min(self.max_delay)
    }

    pub fn reset_attempts(&mut self) {
        self.current_attempt = 0;
        self.last_reconnect_time = None;
    }
}

impl RoomCardState {
    pub fn new(room_id: u64) -> Self {
        Self {
            room_id,
            status: RoomCardStatus::default(),
            entity: None,
            room_info: None,
            user_info: None,
            user_stop: false,
            downloader: None,
            downloader_status: None,
            reconnecting: false,
            reconnect_manager: ReconnectManager::new(
                10,
                Duration::from_secs(1),
                Duration::from_secs(30),
            ),
        }
    }
}

pub struct AppState {
    pub client: HttpClient,
    pub room_states: Vec<RoomCardState>,
    pub settings: GlobalSettings,
}

impl AppState {
    pub fn init(cx: &mut App) {
        log_user_action("初始化应用状态", None);

        let client = HttpClient::new(cx.http_client());
        let mut global_settings = GlobalSettings::load();

        log_config_change("录制目录", &global_settings.record_dir);
        log_config_change("默认录制质量", &format!("{}", global_settings.quality));
        log_config_change("默认录制格式", &format!("{}", global_settings.format));
        log_config_change("默认编码格式", &format!("{}", global_settings.codec));
        log_config_change("主题", &global_settings.theme_name);

        if !global_settings.rooms.is_empty() {
            log_user_action(
                "加载已保存的房间",
                Some(&format!("共{}个房间", global_settings.rooms.len())),
            );

            let settings = global_settings.clone();
            for room_settings in global_settings.rooms.iter_mut() {
                *room_settings = room_settings.merge_global(&settings)
            }
        }

        let state = Self {
            client,
            settings: global_settings,
            room_states: vec![],
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

    pub fn add_room(&mut self, settings: RoomSettings) {
        self.settings.rooms.push(settings);
    }

    pub fn has_room(&self, room_id: u64) -> bool {
        self.settings
            .rooms
            .iter()
            .any(|settings| settings.room_id == room_id)
    }

    pub fn get_room_settings(&self, room_id: u64) -> Option<&RoomSettings> {
        self.settings
            .rooms
            .iter()
            .find(|settings| settings.room_id == room_id)
    }

    pub fn get_room_state(&self, room_id: u64) -> Option<&RoomCardState> {
        self.room_states
            .iter()
            .find(|state| state.room_id == room_id)
    }

    pub fn get_room_state_mut(&mut self, room_id: u64) -> Option<&mut RoomCardState> {
        self.room_states
            .iter_mut()
            .find(|state| state.room_id == room_id)
    }

    pub fn add_room_state(&mut self, room_id: u64) {
        if !self
            .room_states
            .iter()
            .any(|state| state.room_id == room_id)
        {
            self.room_states.push(RoomCardState::new(room_id));
        }
    }

    pub fn has_room_state(&self, room_id: u64) -> bool {
        self.room_states
            .iter()
            .any(|state| state.room_id == room_id)
    }

    pub fn remove_room_state(&mut self, room_id: u64) {
        self.room_states.retain(|state| state.room_id != room_id);
    }
}

impl Global for AppState {}
