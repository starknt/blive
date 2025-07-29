use gpui::{App, Entity, Global, Task, WeakEntity};
use std::collections::HashMap;

use crate::{api::HttpClient, components::RoomCard, settings::GlobalSettings};

pub enum RecordingTaskStatus {
    Idle,
    Recording,
    Paused,
    Error(String),
}

pub struct RecordingTask {
    pub status: RecordingTaskStatus,
    pub task: Task<anyhow::Result<()>>,
    pub entity: WeakEntity<RoomCard>,
}

impl RecordingTask {
    pub fn new(entity: WeakEntity<RoomCard>, task: Task<anyhow::Result<()>>) -> Self {
        Self {
            entity,
            task,
            status: RecordingTaskStatus::Idle,
        }
    }

    pub fn update_status(&mut self, status: RecordingTaskStatus) {
        self.status = status;
    }
}

pub struct AppState {
    pub client: HttpClient,
    pub room_entities: Vec<Entity<RoomCard>>,
    pub settings: GlobalSettings,
    pub recording_tasks: HashMap<u64, RecordingTask>,
}

impl AppState {
    pub fn init(cx: &mut App) {
        let client = HttpClient::new(cx.http_client());
        let state = Self {
            client,
            room_entities: vec![],
            settings: GlobalSettings::load(),
            recording_tasks: HashMap::new(),
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
