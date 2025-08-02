use gpui::{ClickEvent, Entity, MouseButton, Subscription, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, ContextModal, IconName, Sizable, StyledExt, Theme, ThemeMode, TitleBar,
    badge::Badge,
    button::{Button, ButtonVariants},
    scroll::ScrollbarShow,
};

use crate::{components::AppSettings, themes::ThemeSwitcher};

pub struct AppTitleBar {
    title: String,
    theme_switcher: Entity<ThemeSwitcher>,
    settings: Entity<AppSettings>,
    _subscriptions: Vec<Subscription>,
}

impl AppTitleBar {
    pub fn new(title: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if cx.should_auto_hide_scrollbars() {
            Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Scrolling;
        } else {
            Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Hover;
        }

        let theme_switcher = cx.new(|cx| ThemeSwitcher::new(cx));
        let settings = cx.new(|cx| AppSettings::new(window, cx));

        Self {
            title,
            theme_switcher,
            settings,
            _subscriptions: vec![],
        }
    }

    fn change_color_mode(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let mode = match cx.theme().mode.is_dark() {
            true => ThemeMode::Light,
            false => ThemeMode::Dark,
        };

        Theme::change(mode, None, cx);
    }
}

impl Render for AppTitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notifications_count = window.notifications(cx).len();

        TitleBar::new()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_bold()
                            .text_lg()
                            .text_color(cx.theme().primary)
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().accent_foreground)
                            .child("关注早早碎谢谢喵"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_4()
                    .gap_3()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(self.settings.clone())
                    .child(self.theme_switcher.clone())
                    .child(
                        Button::new("theme-mode")
                            .map(|this| {
                                if cx.theme().mode.is_dark() {
                                    this.icon(IconName::Sun)
                                } else {
                                    this.icon(IconName::Moon)
                                }
                            })
                            .small()
                            .ghost()
                            .rounded_full()
                            .on_click(cx.listener(Self::change_color_mode)),
                    )
                    .child(
                        div().relative().child(
                            Badge::new().count(notifications_count).max(99).child(
                                Button::new("bell")
                                    .small()
                                    .ghost()
                                    .compact()
                                    .rounded_full()
                                    .icon(IconName::Bell),
                            ),
                        ),
                    ),
            )
    }
}
