use crate::{
    settings::{GlobalSettings, Quality, Strategy, StreamCodec, VideoContainer},
    state::AppState,
};
use gpui::{App, ClickEvent, Entity, EventEmitter, Subscription, Window, prelude::*};
use gpui_component::{
    ContextModal, StyledExt,
    button::{Button, ButtonVariants},
    dropdown::{Dropdown, DropdownState},
    h_flex,
    input::{InputEvent, InputState, TextInput},
    notification::Notification,
    text::Text,
    v_flex,
};

pub struct SettingsModal {
    global_settings: GlobalSettings,
    record_dir_input: Entity<InputState>,
    strategy_input: Entity<DropdownState<Vec<String>>>,
    quality_input: Entity<DropdownState<Vec<String>>>,
    format_input: Entity<DropdownState<Vec<String>>>,
    codec_input: Entity<DropdownState<Vec<String>>>,
    _subscriptions: Vec<Subscription>,
    lock: bool,
}

#[derive(Debug, Clone)]
pub enum SettingsModalEvent {
    SaveSettings(GlobalSettings),
    QuitSettings,
}

impl EventEmitter<SettingsModalEvent> for SettingsModal {}

impl SettingsModal {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let global_settings = AppState::global(cx).settings.clone();

        let record_dir_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("录制目录路径")
                .default_value(global_settings.record_dir.clone())
        });

        let strategy_input = cx.new(|cx| {
            let mut state = DropdownState::new(
                vec![
                    Strategy::LowCost.to_string(),
                    Strategy::PriorityConfig.to_string(),
                ],
                Some(0),
                window,
                cx,
            );

            state.set_selected_value(&global_settings.strategy.to_string(), window, cx);

            state
        });

        let quality_input = cx.new(|cx| {
            let mut state = DropdownState::new(
                vec![
                    Quality::Dolby.to_string(),
                    Quality::UHD4K.to_string(),
                    Quality::Original.to_string(),
                    Quality::BlueRay.to_string(),
                    Quality::UltraHD.to_string(),
                    Quality::HD.to_string(),
                    Quality::Smooth.to_string(),
                ],
                Some(0),
                window,
                cx,
            );

            state.set_selected_value(&global_settings.quality.to_string(), window, cx);

            state
        });

        let format_input = cx.new(|cx| {
            let mut state = DropdownState::new(
                vec![
                    VideoContainer::FLV.to_string(),
                    VideoContainer::FMP4.to_string(),
                    VideoContainer::TS.to_string(),
                ],
                Some(0),
                window,
                cx,
            );

            state.set_selected_value(&global_settings.format.to_string(), window, cx);

            state
        });

        let codec_input = cx.new(|cx| {
            let mut state = DropdownState::new(
                vec!["avc".to_string(), "hevc".to_string()],
                Some(0),
                window,
                cx,
            );

            state.set_selected_value(&global_settings.codec.to_string(), window, cx);

            state
        });

        let _subscriptions =
            vec![cx.subscribe_in(&record_dir_input, window, Self::on_record_dir_input_change)];

        Self {
            global_settings,
            record_dir_input,
            strategy_input,
            quality_input,
            format_input,
            codec_input,
            _subscriptions,
            lock: false,
        }
    }

    fn on_record_dir_input_change(
        &mut self,
        this: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.lock {
            self.lock = false;
            return;
        }

        if let InputEvent::Change(value) = event {
            this.update(cx, |this, cx| {
                self.lock = true;
                this.set_value(value, window, cx);
            });
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn save_settings(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let record_dir = self.record_dir_input.read(cx).value();
        let quality_str = self.quality_input.read(cx).selected_value();
        let format = self.format_input.read(cx).selected_value();
        let codec = self.codec_input.read(cx).selected_value();

        self.global_settings.record_dir = record_dir.to_string();

        // 解析质量设置
        if let Some(quality_str) = quality_str {
            let quality = match quality_str.as_str() {
                "杜比" => Quality::Dolby,
                "4K" => Quality::UHD4K,
                "原画" => Quality::Original,
                "蓝光" => Quality::BlueRay,
                "超清" => Quality::UltraHD,
                "高清" => Quality::HD,
                "流畅" => Quality::Smooth,
                _ => Quality::Original,
            };
            self.global_settings.quality = quality;
        };

        if let Some(format) = format {
            self.global_settings.format = match format.as_str() {
                "flv" => VideoContainer::FLV,
                "fmp4" => VideoContainer::FMP4,
                "ts" => VideoContainer::TS,
                _ => VideoContainer::FMP4,
            };
        }

        if let Some(codec) = codec {
            self.global_settings.codec = match codec.as_str() {
                "avc" => StreamCodec::AVC,
                "hevc" => StreamCodec::HEVC,
                _ => StreamCodec::AVC,
            };
        }

        cx.emit(SettingsModalEvent::SaveSettings(
            self.global_settings.clone(),
        ));

        window.push_notification(Notification::success("设置保存成功"), cx);
    }

    pub fn quit_settings(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(SettingsModalEvent::QuitSettings);
    }

    fn open_dir(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            if let Some(handle) = rfd::AsyncFileDialog::new().pick_folder().await {
                let value = handle.path().to_string_lossy().to_string();

                let _ = this.update(cx, |this, cx| {
                    this.record_dir_input.update(cx, |_, cx| {
                        cx.emit(InputEvent::Change(value.into()));
                    });
                });
            }
        })
        .detach();
    }
}

impl Render for SettingsModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_y_4()
            .child(
                v_flex().gap_y_5().child(
                    v_flex()
                        .gap_2()
                        .child(
                            v_flex()
                                .gap_y_2()
                                .child(Text::String("录制目录".into()))
                                .child(
                                    h_flex()
                                        .gap_x_4()
                                        .child(
                                            TextInput::new(&self.record_dir_input).disabled(true),
                                        )
                                        .child(
                                            Button::new("open_dir")
                                                .label("选择目录")
                                                .primary()
                                                .on_click(cx.listener(Self::open_dir)),
                                        ),
                                ),
                        )
                        .child(
                            v_flex()
                                .font_bold()
                                .gap_2()
                                .child(Text::String("录制策略".into()))
                                .child(Dropdown::new(&self.strategy_input).max_w_32()),
                        )
                        .child(
                            v_flex()
                                .font_bold()
                                .gap_2()
                                .child(Text::String("录制质量".into()))
                                .child(Dropdown::new(&self.quality_input).max_w_32()),
                        )
                        .child(
                            v_flex()
                                .font_bold()
                                .gap_2()
                                .child(Text::String("录制格式".into()))
                                .child(Dropdown::new(&self.format_input).max_w_32()),
                        )
                        .child(
                            v_flex()
                                .font_bold()
                                .gap_2()
                                .child(Text::String("录制编码".into()))
                                .child(Dropdown::new(&self.codec_input).max_w_32()),
                        ),
                ),
            )
            .child(h_flex().justify_end().gap_x_4().children(vec![
                    Button::new("save")
                        .label("保存设置")
                        .primary()
                        .on_click(cx.listener(Self::save_settings)),
                    Button::new("quit")
                        .label("退出设置")
                        .warning()
                        .on_click(cx.listener(Self::quit_settings)),
                ]))
    }
}
