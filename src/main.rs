use blive::logger::create_default_logger;
use blive::settings::{APP_NAME, DISPLAY_NAME};
use blive::{app::BLiveApp, assets::Assets, state::AppState, themes::ThemeSwitcher};
use gpui::{
    App, Application, Bounds, KeyBinding, WindowBounds, WindowKind, WindowOptions, actions,
    prelude::*, px, size,
};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};
use gpui_component::{Root, TitleBar, theme};
use reqwest_client::ReqwestClient;

actions!(menu, [Quit]);

fn main() {
    // 初始化日志系统
    let logger = create_default_logger().expect("无法创建日志管理器");
    logger.init().expect("无法初始化日志系统");
    logger.log_app_start(env!("CARGO_PKG_VERSION"));

    let app = Application::new().with_assets(Assets);

    app.on_reopen(|cx| {
        open_main_window(cx);
    });

    app.run(move |cx| {
        gpui_component::init(cx);

        let http_client = std::sync::Arc::new(ReqwestClient::user_agent("blive/0.1.0").unwrap());
        cx.set_http_client(http_client);

        AppState::init(cx);
        theme::init(cx);
        ThemeSwitcher::init(cx);
        BLiveApp::init(cx);

        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

        cx.on_action(|_: &Quit, cx: &mut App| {
            cx.quit();
        });

        cx.on_app_quit(move |cx| {
            cx.read_global(|state: &AppState, _| {
                state.settings.save();
            });

            // 记录应用关闭日志
            logger.log_app_shutdown();

            async {}
        })
        .detach();

        #[cfg(target_os = "macos")]
        cx.set_menus(vec![Menu {
            name: APP_NAME.into(),
            items: vec![MenuItem::action("退出", Quit)],
        }]);

        cx.activate(true);

        open_main_window(cx);
    });
}

fn open_main_window(cx: &mut App) {
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
                let root = BLiveApp::view(DISPLAY_NAME.into(), window, cx);

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
}
