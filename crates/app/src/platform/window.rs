#[cfg(target_os = "macos")]
use gpui_kit::Point;
use gpui_kit::{px, size, App, TitlebarOptions, WindowBounds, WindowOptions};

pub fn default_window_options(cx: &mut App) -> WindowOptions {
    let mut options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1240.), px(820.)), cx)),
        app_owns_titlebar_drag: true,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    {
        options.titlebar = Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(Point {
                x: px(12.0),
                y: px(13.0),
            }),
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        options.titlebar = Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        });
    }

    options
}
