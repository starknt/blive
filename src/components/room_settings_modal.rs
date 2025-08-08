use crate::settings::{Quality, RoomSettings, Strategy, StreamCodec, VideoContainer};
use gpui::{App, ClickEvent, Entity, EventEmitter, Subscription, Window, prelude::*};
use gpui_component::{
    ContextModal, IndexPath, StyledExt,
    button::{Button, ButtonVariants},
    dropdown::{Dropdown, DropdownState},
    h_flex,
    input::{InputState, TextInput},
    notification::Notification,
    text::Text,
    v_flex,
};

pub struct RoomSettingsModal {
    settings: RoomSettings,
    record_name_input: Entity<InputState>,
    strategy_input: Entity<DropdownState<Vec<String>>>,
    quality_input: Entity<DropdownState<Vec<String>>>,
    format_input: Entity<DropdownState<Vec<String>>>,
    codec_input: Entity<DropdownState<Vec<String>>>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone)]
pub enum RoomSettingsModalEvent {
    SaveSettings(RoomSettings),
    QuitSettings,
}

impl EventEmitter<RoomSettingsModalEvent> for RoomSettingsModal {}

impl RoomSettingsModal {
    pub fn new(settings: RoomSettings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let record_name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("录制文件名")
                .default_value(settings.record_name.clone())
        });

        let strategy_input = cx.new(|cx| {
            let mut state = DropdownState::new(
                vec![
                    Strategy::LowCost.to_string(),
                    Strategy::PriorityConfig.to_string(),
                ],
                Some(IndexPath::new(0)),
                window,
                cx,
            );

            state.set_selected_value(
                &settings.strategy.unwrap_or_default().to_string(),
                window,
                cx,
            );

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
                Some(IndexPath::new(0)),
                window,
                cx,
            );

            state.set_selected_value(
                &settings.quality.unwrap_or_default().to_string(),
                window,
                cx,
            );

            state
        });

        let format_input = cx.new(|cx| {
            let mut state = DropdownState::new(
                vec![
                    VideoContainer::FLV.to_string(),
                    VideoContainer::FMP4.to_string(),
                    VideoContainer::TS.to_string(),
                ],
                Some(IndexPath::new(0)),
                window,
                cx,
            );

            state.set_selected_value(&settings.format.unwrap_or_default().to_string(), window, cx);

            state
        });

        let codec_input = cx.new(|cx| {
            let mut state = DropdownState::new(
                vec!["avc".to_string(), "hevc".to_string()],
                Some(IndexPath::new(0)),
                window,
                cx,
            );

            state.set_selected_value(&settings.codec.unwrap_or_default().to_string(), window, cx);

            state
        });

        let _subscriptions = vec![];

        Self {
            settings,
            record_name_input,
            strategy_input,
            quality_input,
            format_input,
            codec_input,
            _subscriptions,
        }
    }

    pub fn view(settings: RoomSettings, window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(settings, window, cx))
    }

    pub fn save_settings(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let strategy_str = self.strategy_input.read(cx).selected_value();
        let quality_str = self.quality_input.read(cx).selected_value();
        let format = self.format_input.read(cx).selected_value();
        let codec = self.codec_input.read(cx).selected_value();

        // 策略设置
        if let Some(strategy_str) = strategy_str {
            let strategy = match strategy_str.as_str() {
                "低占用" => Strategy::LowCost,
                "配置优先" => Strategy::PriorityConfig,
                _ => Strategy::LowCost,
            };
            self.settings.strategy = Some(strategy);
        }

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

            self.settings.quality = Some(quality);
        };

        if let Some(format) = format {
            self.settings.format = match format.as_str() {
                "flv" => Some(VideoContainer::FLV),
                "fmp4" => Some(VideoContainer::FMP4),
                "ts" => Some(VideoContainer::TS),
                _ => Some(VideoContainer::FMP4),
            };
        }

        if let Some(codec) = codec {
            self.settings.codec = match codec.as_str() {
                "avc" => Some(StreamCodec::AVC),
                "hevc" => Some(StreamCodec::HEVC),
                _ => Some(StreamCodec::AVC),
            };
        }

        cx.emit(RoomSettingsModalEvent::SaveSettings(self.settings.clone()));
        window.push_notification(Notification::success("设置保存成功"), cx);
    }

    pub fn quit_settings(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(RoomSettingsModalEvent::QuitSettings);
    }
}

impl Render for RoomSettingsModal {
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
                                .child(Text::String("录制文件名".into()))
                                .child(TextInput::new(&self.record_name_input)),
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
