#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use blive::logger::{init_logger, log_app_shutdown, log_app_start};
use blive::settings::{APP_NAME, DISPLAY_NAME};
use blive::tray::{SystemTray, TrayMessage};
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
    #[cfg(debug_assertions)]
    ffmpeg_sidecar::download::auto_download().expect("无法自动下载 ffmpeg");

    init_logger().expect("无法初始化日志系统");
    log_app_start(env!("CARGO_PKG_VERSION"));

    let quiting = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (tx, rx) = flume::unbounded();
    let mut system_tray = SystemTray::new();

    let open_main_window_tx = tx.clone();
    system_tray.add_menu_item("打开主窗口", move || {
        // This can be used to open the main application window
        open_main_window_tx.send(TrayMessage::OpenWindow).unwrap();
    });

    let quit_app_tx = tx.clone();
    system_tray.add_menu_item("退出应用", move || {
        // Send a quit message to the application
        quit_app_tx.send(TrayMessage::Quit).unwrap();
    });

    let app = Application::new().with_assets(Assets);
    app.on_reopen(|cx| {
        open_main_window(cx);
    });

    let app_quitting = quiting.clone();
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
            let downloaders = cx.read_global(|state: &AppState, _| {
                state.settings.save();

                state.downloaders.clone()
            });

            let app_quitting = app_quitting.clone();
            async move {
                // Wait for all downloaders to stop
                futures::future::join_all(downloaders.iter().map(|downloader| downloader.stop())).await;

                // 记录应用关闭日志
                log_app_shutdown();
                app_quitting.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        })
        .detach();

        #[cfg(target_os = "macos")]
        cx.set_menus(vec![Menu {
            name: APP_NAME.into(),
            items: vec![MenuItem::action("退出", Quit)],
        }]);

        open_main_window(cx);
        cx.activate(true);


        cx.spawn(async move |cx| {
            loop {
                if let Ok(event) = rx.try_recv() {
                    match event {
                        TrayMessage::Quit => {
                            let _ = cx.update(|cx| {
                                cx.quit();
                            });
                            break;
                        }
                        TrayMessage::OpenWindow => {
                            let _ = cx.update(|cx| {
                                if cx.windows().is_empty() {
                                    // open main window
                                    open_main_window(cx);
                                } else {
                                    // If the main window is already open, just activate it
                                    if let Some(window) = cx.windows().first() {
                                        window
                                            .update(cx, |_, window, _| {
                                                #[cfg(windows)] {
                                                    unsafe  {
                                                        use windows::Win32::Foundation::*;
                                                        use raw_window_handle::HasWindowHandle;
                                                        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_RESTORE};

                                                        if let Ok(handle) = window.window_handle()
                                                            && let raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw() {
                                                                // If the window is minimized, restore it
                                                                let _ = ShowWindow(HWND(handle.hwnd.get() as *mut std::ffi::c_void), SW_RESTORE);
                                                            }

                                                    }
                                                }
                                            })
                                            .expect("Failed to activate window");
                                    }
                                }
                            });
                        }
                    }
                }

                cx.background_executor().timer(Duration::from_secs(2)).await;
            }
        })
        .detach();
    });

    // loop {
    //     if quiting.load(std::sync::atomic::Ordering::Relaxed) {
    //         system_tray.quit();
    //         break;
    //     }

    //     std::thread::sleep(std::time::Duration::from_secs(3));
    // }
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

                // window.on_window_should_close(cx, |w, _| {
                //     w.minimize_window();

                //     false
                // });

                cx.new(|cx| Root::new(root.into(), window, cx))
            })
            .expect("Failed to open window");

        window
            .update(cx, |_, window, _| {
                window.set_window_title(DISPLAY_NAME);
                window.activate_window();
            })
            .expect("Failed to update window");
    })
    .detach();
}
