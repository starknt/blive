use gpui::{ClickEvent, Corner, Entity, Hsla, MouseButton, Subscription, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, IconName, Sizable, Theme, ThemeMode, TitleBar,
    button::{Button, ButtonVariants},
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    scroll::ScrollbarShow,
};

pub struct AppTitleBar {
    title: String,
    theme_color: Entity<ColorPickerState>,
    _subscriptions: Vec<Subscription>,
}

impl AppTitleBar {
    pub fn new(title: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if cx.should_auto_hide_scrollbars() {
            Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Scrolling;
        } else {
            Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Hover;
        }

        let theme_color = cx.new(|cx| ColorPickerState::new(window, cx));

        let _subscriptions = vec![cx.subscribe_in(
            &theme_color,
            window,
            |this, _, ev: &ColorPickerEvent, window, cx| match ev {
                ColorPickerEvent::Change(color) => {
                    this.set_theme_color(*color, window, cx);
                }
            },
        )];

        Self {
            title,
            theme_color,
            _subscriptions,
        }
    }

    fn set_theme_color(
        &mut self,
        color: Option<Hsla>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(color) = color {
            let theme = cx.global_mut::<Theme>();
            theme.apply_color(color);
            self.theme_color.update(cx, |state, cx| {
                state.set_value(color, window, cx);
            });
            window.refresh();
        }
    }

    fn change_color_mode(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let mode = match cx.theme().mode.is_dark() {
            true => ThemeMode::Light,
            false => ThemeMode::Dark,
        };

        Theme::change(mode, None, cx);
        self.set_theme_color(self.theme_color.read(cx).value(), window, cx);
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
                    .child(
                        ColorPicker::new(&self.theme_color)
                            .small()
                            .anchor(Corner::TopRight)
                            .icon(IconName::Palette),
                    )
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
