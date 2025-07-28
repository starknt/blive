use blive::settings::{APP_NAME, DISPLAY_NAME};
use blive::{LiveRecoderApp, assets::Assets, state::AppState, themes::ThemeSwitcher};
use gpui::{
    App, Application, Bounds, KeyBinding, WindowBounds, WindowKind, WindowOptions, actions,
    prelude::*, px, size,
};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};
use gpui_component::{Root, TitleBar, theme};
use tracing_subscriber::prelude::*;

actions!(menu, [Quit]);

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("gpui_component=trace".parse().unwrap())
                .add_directive("reqwest_client=trace".parse().unwrap())
                .add_directive("recoder=trace".parse().unwrap()),
        )
        .init();

    let app = Application::new().with_assets(Assets);
    let version = env!("CARGO_PKG_VERSION");

    app.run(move |cx| {
        gpui_component::init(cx);

        let http_client = std::sync::Arc::new(
            reqwest_client::ReqwestClient::user_agent(&format!("{APP_NAME}/{version}")).unwrap(),
        );
        cx.set_http_client(http_client);

        AppState::init(cx);
        theme::init(cx);
        ThemeSwitcher::init(cx);
        LiveRecoderApp::init(cx);

        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

        cx.on_action(|_: &Quit, cx: &mut App| {
            cx.quit();
        });

        cx.on_app_quit(move |cx| {
            cx.read_global(|state: &AppState, _| {
                state.settings.save();
            });

            async {}
        })
        .detach();

        #[cfg(target_os = "macos")]
        cx.set_menus(vec![Menu {
            name: APP_NAME.into(),
            items: vec![MenuItem::action("退出", Quit)],
        }]);

        cx.activate(true);

        let mut window_size = size(px(1600.0), px(900.0));
        if let Some(display) = cx.primary_display() {
            let display_size = display.bounds().size;
            window_size.width = window_size.width.min(display_size.width * 0.85);
            window_size.height = window_size.height.min(display_size.height * 0.85);
        }
        let window_bounds = Bounds::centered(None, window_size, cx);

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                app_id: Some(APP_NAME.into()),
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                titlebar: Some(TitleBar::title_bar_options()),
                window_min_size: Some(gpui::Size {
                    width: px(640.),
                    height: px(480.),
                }),
                kind: WindowKind::Normal,
                #[cfg(not(target_os = "linux"))]
                window_background: gpui::WindowBackgroundAppearance::Blurred,
                #[cfg(target_os = "linux")]
                window_background: gpui::WindowBackgroundAppearance::Transparent,
                #[cfg(target_os = "linux")]
                window_decorations: Some(gpui::WindowDecorations::Client),
                ..Default::default()
            };

            let window = cx
                .open_window(options, |window, cx| {
                    let root = LiveRecoderApp::view(DISPLAY_NAME.into(), window, cx);

                    cx.new(|cx| Root::new(root.into(), window, cx))
                })
                .expect("Failed to open window");

            window
                .update(cx, |_, window, _| {
                    window.activate_window();
                })
                .expect("Failed to update window");
        })
        .detach();
    });
}
