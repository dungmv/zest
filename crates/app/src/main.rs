#![recursion_limit = "512"]
#![windows_subsystem = "windows"]

mod app;
mod components;
mod platform;
pub mod services;
pub mod settings;
mod state;
mod theme;
pub mod version;

use app::*;
use gpui_kit::assets::AllAssets;
use gpui_kit::*;
use platform::default_window_options;

/// Appends every panic message with a backtrace to a log file.
///
/// The default hook only prints to stderr, which users never see in a GUI
/// app, and a panic that aborts during reporting (or an `extern "system"`
/// unwind boundary) loses the message entirely — the process just dies with
/// `STATUS_STACK_BUFFER_OVERRUN`. Writing to a file from the hook preserves
/// the original panic site for crash reports.
fn install_panic_logger() {
    let log_path = std::env::temp_dir().join("zest-panic.log");
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .and_then(|mut file| {
                use std::io::Write as _;
                writeln!(file, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
                    .and_then(|_| {
                        writeln!(
                            file,
                            "[{}] panic on thread '{}'",
                            unix_timestamp(),
                            thread_name
                        )
                    })
                    .and_then(|_| writeln!(file, "{}", info))
                    .and_then(|_| writeln!(file, "backtrace:\n{}", backtrace))
            });
    }));
}

/// Seconds since the Unix epoch; no chrono dependency needed for a log line.
fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn main() {
    install_panic_logger();

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

        let (_window, app_view) = gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| AppView::new(window, cx))
        })
        .expect("Failed to open main window");

        let modal = app_view.read(cx).state.modal.clone();
        platform::init_app_menu(modal, cx);
    });
}
