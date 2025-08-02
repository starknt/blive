#![allow(clippy::collapsible_if)]

pub mod app;
pub mod assets;
pub mod components;
pub mod core;
pub mod error;
pub mod logger;
pub mod settings;
pub mod state;
pub mod themes;
pub mod title_bar;

pub use logger::{
    LogLevel, log_app_shutdown, log_app_start, log_config_change, log_network_request,
    log_network_response, log_recording_error, log_recording_start, log_recording_stop,
    log_user_action, set_log_level,
};
