use std::sync::{Arc, atomic};

use crate::{
    components::{SettingsModal, SettingsModalEvent},
    state::AppState,
};
use gpui::{App, Entity, FocusHandle, Focusable, Subscription, Window, div, prelude::*};
use gpui_component::{
    ContextModal, Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    text::Text,
};

pub struct AppSettings {
    show: Arc<atomic::AtomicBool>,
    focus_handle: FocusHandle,
    setting_modal: Entity<SettingsModal>,
    _subscriptions: Vec<Subscription>,
}

impl AppSettings {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let setting_modal = SettingsModal::view(window, cx);

        let show = Arc::new(atomic::AtomicBool::new(false));

        Self {
            show,
            focus_handle: cx.focus_handle(),
            _subscriptions: vec![cx.subscribe_in(
                &setting_modal,
                window,
                Self::on_setting_modal_event,
            )],
            setting_modal,
        }
    }

    fn on_setting_modal_event(
        &mut self,
        _this: &Entity<SettingsModal>,
        event: &SettingsModalEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SettingsModalEvent::SaveSettings(settings) => {
                AppState::global_mut(cx).settings = settings.clone();
                settings.save();
            }
            SettingsModalEvent::QuitSettings => {
                self.show.store(false, atomic::Ordering::Relaxed);
                window.close_modal(cx);
            }
        }
    }

    fn show_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show.load(atomic::Ordering::Relaxed) {
            return;
        }

        let setting_modal = self.setting_modal.clone();
        let show = self.show.clone();
        window.open_modal(cx, move |modal, _window, _cx| {
            show.store(true, atomic::Ordering::Relaxed);
            let show = show.clone();

            modal
                .rounded_lg()
                .title(
                    div()
                        .font_bold()
                        .text_2xl()
                        .child(Text::String("全局设置".into())),
                )
                .overlay_closable(false)
                .child(setting_modal.clone())
                .on_close(move |_, _, _| show.store(false, atomic::Ordering::Relaxed))
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
        let show = self.show.clone();

        div().track_focus(&self.focus_handle).child(
            Button::new("settings")
                .icon(IconName::Settings)
                .ghost()
                .small()
                .disabled(show.load(atomic::Ordering::Relaxed))
                .on_click(cx.listener(|this, _, window, cx| this.show_modal(window, cx))),
        )
    }
}
