#![recursion_limit = "512"]
#![windows_subsystem = "windows"]

mod app;
mod components;
mod platform;
pub mod services;
pub mod settings;
mod state;
mod theme;

use app::*;
use gpui_kit::assets::AllAssets;
use gpui_kit::*;
use platform::default_window_options;

fn main() {
    #[cfg(target_os = "macos")]
    {
        // Suppress benign internal Apple InputMethodKit / TSM logs
        // (such as "ERROR: getApplicationProperty: called with invalid property")
        // emitted when typing with macOS Vietnamese Telex / CJK input methods.
        if std::env::var("OS_ACTIVITY_MODE").is_err() {
            std::env::set_var("OS_ACTIVITY_MODE", "disable");
        }
    }

    let app = gpui_kit::application().with_assets(AllAssets);

    app.run(move |cx| {
        platform::set_dock_icon();
        gpui_kit::init(cx);

        let settings = settings::AppSettings::load();
        let pref = theme::ThemePreference::from_str_opt(settings.theme.as_deref());
        let mode = theme::resolve_theme_mode(pref, cx.window_appearance());
        theme::set_theme(pref, mode);
        theme::sync_component_theme(mode, cx);

        // Register keyboard shortcuts
        cx.bind_keys([
            KeyBinding::new("ctrl-enter", SendRequest, None),
            KeyBinding::new("cmd-enter", SendRequest, None),
            KeyBinding::new("ctrl-k", ToggleCommandPalette, None),
            KeyBinding::new("cmd-k", ToggleCommandPalette, None),
            KeyBinding::new("ctrl-b", ToggleSidebar, None),
            KeyBinding::new("cmd-b", ToggleSidebar, None),
            KeyBinding::new("ctrl-,", ToggleSettings, None),
            KeyBinding::new("cmd-,", ToggleSettings, None),
            KeyBinding::new("ctrl-shift-g", ToggleGitModal, None),
            KeyBinding::new("cmd-shift-g", ToggleGitModal, None),
            KeyBinding::new("ctrl-i", OpenImportModal, None),
            KeyBinding::new("cmd-i", OpenImportModal, None),
            KeyBinding::new("ctrl-[", NavigateBack, None),
            KeyBinding::new("cmd-[", NavigateBack, None),
            KeyBinding::new("ctrl-]", NavigateForward, None),
            KeyBinding::new("cmd-]", NavigateForward, None),
            KeyBinding::new("escape", DismissModal, None),
        ]);

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let window_options = default_window_options(cx);

        gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| AppView::new(window, cx))
        })
        .expect("Failed to open main window");
    });
}
