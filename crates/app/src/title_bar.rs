use gpui::{ClickEvent, Entity, MouseButton, Subscription, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, IconName, Sizable, Theme, ThemeMode, TitleBar,
    button::{Button, ButtonVariants},
    scroll::ScrollbarShow,
};

use crate::themes::ThemeSwitcher;

pub struct AppTitleBar {
    title: String,
    theme_switcher: Entity<ThemeSwitcher>,
    _subscriptions: Vec<Subscription>,
}

impl AppTitleBar {
    pub fn new(title: String, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        if cx.should_auto_hide_scrollbars() {
            Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Scrolling;
        } else {
            Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Hover;
        }

        let theme_switcher = cx.new(|cx| ThemeSwitcher::new(cx));

        Self {
            title,
            theme_switcher,
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new()
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(self.title.clone() + " - " + "关注早早碎谢谢喵"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_2()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                            .on_click(cx.listener(Self::change_color_mode)),
                    ),
            )
    }
}
