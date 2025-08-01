use crate::{
    components::{SettingsModal, SettingsModalEvent},
    state::AppState,
};
use gpui::{App, Entity, FocusHandle, Focusable, Subscription, Window, div, prelude::*};
use gpui_component::{
    ContextModal, IconName, Sizable,
    button::{Button, ButtonVariants},
};

pub struct AppSettings {
    focus_handle: FocusHandle,
    setting_modal: Entity<SettingsModal>,
    _subscriptions: Vec<Subscription>,
}

impl AppSettings {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let setting_modal = SettingsModal::view(window, cx);

        Self {
            focus_handle: cx.focus_handle(),
            _subscriptions: vec![cx.subscribe(&setting_modal, Self::on_setting_modal_event)],
            setting_modal,
        }
    }

    fn on_setting_modal_event(
        _this: Entity<SettingsModal>,
        event: &SettingsModalEvent,
        cx: &mut App,
    ) {
        match event {
            SettingsModalEvent::SaveSettings(settings) => {
                AppState::global_mut(cx).settings = settings.clone();
                settings.save();
            }
        }
    }

    fn show_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let setting_modal = self.setting_modal.clone();
        window.open_modal(cx, move |modal, _window, _cx| {
            modal
                .title("全局设置")
                .overlay(true)
                .overlay_closable(false)
                .child(setting_modal.clone())
        });
    }
}

impl Focusable for AppSettings {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().track_focus(&self.focus_handle).child(
            Button::new("settings")
                .icon(IconName::Settings)
                .ghost()
                .small()
                .on_click(cx.listener(|this, _, window, cx| this.show_modal(window, cx))),
        )
    }
}
